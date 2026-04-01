# aicmd — Tool System Design

## 1. Core Abstractions (`tools/mod.rs`)

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn definition(&self) -> ToolDefinition;
    async fn execute(&self, arguments: serde_json::Value) -> Result<ToolOutput>;
}

pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

pub struct ToolOutput {
    pub content: String,
}
```

---

## 2. Tool Registry (`tools/mod.rs`)

A `ToolRegistry` holds all available tools and filters them based on CLI flags:

```rust
pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new(cli_flags: &CliFlags) -> Self {
        let mut tools: Vec<Box<dyn Tool>> = Vec::new();

        if !cli_flags.no_tools {
            if !cli_flags.quiet {
                tools.push(Box::new(AskUserTool));
            }
            if !cli_flags.blind {
                tools.push(Box::new(FindCommandTool));
            }
        }

        Self { tools }
    }

    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.iter().map(|t| t.definition()).collect()
    }

    pub async fn execute(&self, name: &str, args: serde_json::Value) -> Result<ToolOutput> {
        self.tools
            .iter()
            .find(|t| t.name() == name)
            .ok_or_else(|| Error::ToolError(format!("unknown tool: {}", name)))?
            .execute(args)
            .await
    }
}
```

### Filtering by CLI flags

| Flag | Tools registered |
|------|-----------------|
| (none) | `ask_user`, `find_command` |
| `-q` | `find_command` only |
| `-b` | `ask_user` only |
| `-n` | Empty registry — no tools |

When the registry is empty, `definitions()` returns `[]` and providers receive no tools in their request.

---

## 3. Tool Definitions (JSON Schema)

Each tool definition is authored once in `ToolDefinition` format and translated into provider-specific wire formats by each provider at request time (see [PROVIDERS.md](./PROVIDERS.md)).

### 3.1 `ask_user`

```rust
ToolDefinition {
    name: "ask_user".into(),
    description: "Ask the user a clarifying question when their request is ambiguous. Present choices as single-letter options so the user can reply with a single character.".into(),
    parameters: serde_json::json!({
        "type": "object",
        "properties": {
            "question": {
                "type": "string",
                "description": "The clarifying question to ask the user."
            },
            "choices": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "key": {
                            "type": "string",
                            "description": "A single-letter key (e.g., 'A', 'B', 'Y', 'N')."
                        },
                        "label": {
                            "type": "string",
                            "description": "A short descriptive label for this choice (e.g., 'use fd', 'use find')."
                        }
                    },
                    "required": ["key", "label"]
                },
                "description": "List of choices, each with a single-letter key and a descriptive label."
            }
        },
        "required": ["question", "choices"]
    }),
}
```

### 3.2 `find_command`

```rust
ToolDefinition {
    name: "find_command".into(),
    description: "Check whether a specific command-line tool is available on the system by running 'command -v' (with 'which' as fallback).".into(),
    parameters: serde_json::json!({
        "type": "object",
        "properties": {
            "command": {
                "type": "string",
                "description": "The command name to look up (e.g., 'fd', 'rg', 'jq')."
            }
        },
        "required": ["command"]
    }),
}
```

---

## 4. Tool Execution

### 4.1 `ask_user` (`tools/ask_user.rs`)

```
1. Receive (question: "Use fd or find?", choices: [{ key: "A", label: "use fd" }, { key: "B", label: "use find" }])
2. Print to stderr:
      > [aicmd] Use fd or find?
        A) use fd
        B) use find
      > Your choice:
3. Read a single line from stdin (blocking, with 60s timeout)
4. Trim whitespace, return as ToolOutput { content: "A" }
```

**Behavioral details:**

- **Timeout**: 60 seconds. If no input is received, return `ToolOutput { content: "(no response)" }` so the model can proceed or ask again.
- **Invalid input**: If the user's input doesn't match any choice key, return the raw input as-is with a note: `ToolOutput { content: "X (not a valid choice)" }`. The model decides how to handle it.
- **Output channel**: All user-facing prompts and interactive text go to **stderr**, keeping stdout clean for the final command output. This ensures pipe usage like `$(aicmd find files)` works correctly.
- **Stdin blocking**: Reading stdin is done in a blocking manner. The tokio runtime should allow blocking on stdin (use `tokio::task::spawn_blocking` or `tokio::io::stdin()`).
- **Display format**: Each choice is displayed as `{key}) {label}`, where `key` is the single-letter identifier and `label` is the model-provided descriptive text.

### 4.2 `find_command` (`tools/find_command.rs`)

```
1. Receive (command: "fd")
2. Run: sh -c "command -v fd"
   - If that fails (exit code 127, command not found in minimal environments), fall back to: which fd
   - Capture stdout and stderr
   - Timeout: 5 seconds
3. If exit code 0 and stdout is non-empty:
      Return ToolOutput { content: "/opt/homebrew/bin/fd" }
4. If exit code non-zero or empty stdout:
      Return ToolOutput { content: "not found" }
5. If command times out:
      Return ToolOutput { content: "error: command lookup timed out" }
```

**Behavioral details:**

- **Shell injection safety**: The `command` parameter is passed as a single argument to `sh -c "command -v $1" -- "$command"`, not interpolated into a shell string. No user-controlled content is substituted into the shell command template.
- **Timeout**: 5 seconds via `tokio::time::timeout`.
- **Environment**: The subprocess inherits the current process environment, so PATH-based lookups work naturally.
- **Primary vs fallback**: `command -v` is POSIX-standard and works on all shells including minimal environments (Docker, CI). `which` is used as a fallback for edge cases where `sh` itself is unavailable.

---

## 5. Provider-Specific Tool Serialization

Tool definitions and results must be translated into each provider's wire format. This translation happens inside each provider implementation (see [PROVIDERS.md — Tool Translation Layer](./PROVIDERS.md#4-tool-translation-layer)).

### Quick reference

| Provider | Tool def key | Tool call in response | Tool result sent back |
|----------|-------------|----------------------|----------------------|
| OpenAI Completions | `tools[].function` | `tool_calls[].function` | `messages[]` with `role: "tool"` |
| Anthropic | `tools[]` | `content[].type: "tool_use"` | `messages[]` with `type: "tool_result"` |
| Gemini | `tools[].function_declarations[]` | `parts[].function_call` | `contents[].parts[].function_response` |
| Responses | `tools[]` (flat) | `output[].type: "function_call"` | `input[].type: "function_call_output"` |

---

## 6. Tool Call Loop

The tool call loop lives in the orchestrator (`orchestrator.rs`). This is the core request/response cycle:

```
messages = [
  Message::System { content: system_prompt },
  Message::User { content: user_prompt },
]

loop (max 10 iterations):
    response = provider.send_request(&messages, &tool_definitions)
    
    if response.content.is_some() && response.tool_calls.is_empty():
        print response.content to stdout
        exit 0
    
    if response.tool_calls.is_not_empty():
        messages.append(Message::Assistant {
            content: response.content,
            tool_calls: response.tool_calls,
        })
        
        for each tool_call in response.tool_calls:
            result = tool_registry.execute(tool_call.name, tool_call.arguments).await
            messages.append(Message::ToolResult {
                tool_call_id: tool_call.id,
                name: tool_call.name,
                content: result.content,
            })
        
        continue loop
    
    error: "empty response from provider"
    exit 1
```

### Key details

- **Single assistant message**: When the response contains both content and tool calls, they are appended as a **single** `Message::Assistant` variant (not two separate messages). This is required by all provider wire formats — the assistant's partial text and its tool calls belong to one message.
- **Message ordering**: The assistant message (containing the tool calls) is appended first, followed by the tool result messages. This matches all provider wire formats — the assistant's tool-call request precedes the tool results in the conversation history.

### Loop invariants

- The orchestrator builds the full conversation history (`Message` variants) and passes it to the provider on each iteration.
- Each provider is responsible for formatting that history into its specific wire format.
- The assistant message (including any partial content + tool calls) is appended to messages before continuing the loop, so the provider sees the full context on the next request.
- The maximum of 10 iterations prevents infinite tool-call loops from uncooperative models.

### Loop exit conditions

| Condition | Action |
|-----------|--------|
| Response has content, no tool calls | Print content to stdout, exit 0 |
| 10 iterations reached without final answer | Print error to stderr, exit 1 |
| Provider returns empty response | Print error to stderr, exit 1 |
| Provider returns error / network failure | Print error to stderr, exit 1 |
| `ask_user` receives EOF on stdin | Print error to stderr, exit 1 |
