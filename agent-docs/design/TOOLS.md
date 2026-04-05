# cmdify — Tool System Design

## 1. Core Abstractions (`tools/mod.rs`)

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn definition(&self) -> ToolDefinition;
    async fn execute(
        &self,
        arguments: serde_json::Value,
        logger: Option<&CmdifyLogger>,
        spinner: Option<&SpinnerPause>,
    ) -> Result<ToolOutput>;
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

## 2. Tool Levels

Tools are organized into numbered levels that provide progressive environment awareness to the model. Each level grants the model additional information about the user's system, improving command accuracy at the cost of increased exposure.

### 2.1 Level Definitions

| Level | Name | Tools | Description |
|-------|------|-------|-------------|
| 0 | none | *(no tools)* | No tool access. The model generates commands from context only. Maximum privacy. |
| 1 | core | `ask_user`, `find_command`, `pwd`, `list_current_directory` | Interactive clarification, command existence checks via PATH, listing files in the current working directory. Read-only, no filesystem writes, no system introspection beyond PATH. |
| 2 | local | `command_help`, `list_any_directory` | Read-only filesystem access: command help/usage text, listing files in any user-specified directory, confirming the working directory. No writes or system introspection. |
| 3 | system | `get_env`, `list_processes` | System introspection: reading environment variables, listing running processes. The model can observe system state beyond the filesystem. Use with caution in sensitive environments. |

**Default level:** 1 (core). This provides a conservative risk profile while remaining useful.

### 2.2 Risk Philosophy

Each level grants the model additional information about the user's environment. More information improves command accuracy but increases the attack surface if the model is malicious or the user is tricked into running cmdify with a compromised prompt.

- **Level 0** — Maximum privacy. The model sees nothing beyond the user's prompt.
- **Level 1** — Safe defaults. The model can ask questions, verify commands exist, and see what files are in the current directory. It cannot read arbitrary paths, see environment variables, or inspect processes.
- **Level 2** — Extended filesystem awareness. The model can read help text (reducing incorrect flag usage), list any directory (useful for "compress my project" type requests), and confirm the working directory. Still no system introspection.
- **Level 3** — Full environment awareness. The model can read environment variables (potentially revealing secrets) and list running processes. Use with caution in sensitive environments.

### 2.3 Configuration

**Precedence:** CLI flag > env var > config file > default (1)

| Source | Format | Example |
|--------|--------|---------|
| CLI flag | `-t N` / `--tools N` | `cmdify -t 2 "compress my project"` |
| Env var | `CMDIFY_TOOL_LEVEL=N` | `CMDIFY_TOOL_LEVEL=2` |
| Config file | `tool_level = N` | `tool_level = 2` in `config.toml` |
| Default | `1` | When no source specifies a level |

Invalid values are clamped to the valid range (0–3).

### 2.4 Legacy Flag Interaction

The existing `-q`/`-b`/`-n` flags are preserved as convenience overrides layered on top of the tool level:

| Flag | Effect |
|------|--------|
| `-q` / `--quiet` | Removes `ask_user` from the active level's tool set |
| `-b` / `--blind` | Removes `find_command` from the active level's tool set |
| `-n` / `--no-tools` | Absolute override — removes all tools regardless of level |

Examples:

| Combination | Active Tools |
|-------------|-------------|
| `-t 1` | `ask_user`, `find_command`, `list_current_directory` |
| `-t 1 -q` | `find_command`, `list_current_directory` |
| `-t 2 -b` | Level 2 tools minus `find_command` |
| `-t 3 -n` | *(none)* |
| `-n` | *(none)* regardless of `-t` |

### 2.5 `--list-tools` Flag

Running `cmdify --list-tools` prints all available tools organized by level to stdout and exits with code 0. Tools that are planned but not yet implemented are shown with a "(not yet implemented)" label.

```
cmdify tool levels (default: 1)

Level 0 — no tools

Level 1 — core:
  ask_user                Ask the user a clarifying question
  find_command            Check whether a command exists on the system
  list_current_directory  List files in the current working directory

Level 2 — local (not yet implemented):
  command_help            Show help text for a command (optional grep filter)
  list_any_directory      List files in any user-specified directory
  pwd                     Print the current working directory

Level 3 — system (not yet implemented):
  get_env                 Read environment variables
  list_processes          List running processes

Use -t N or --tools N to set the tool level (0-3).
Use -q, -b, -n to disable individual tools or all tools.
```

---

## 3. Tool Registry (`tools/mod.rs`)

A `ToolRegistry` holds all active tools based on the tool level and legacy flag overrides:

```rust
pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new(tool_level: u8, quiet: bool, blind: bool, no_tools: bool) -> Self {
        if no_tools {
            return Self { tools: Vec::new() };
        }

        let mut tools: Vec<Box<dyn Tool>> = Vec::new();

        if tool_level >= 1 {
            if !quiet {
                tools.push(Box::new(AskUserTool));
            }
            if !blind {
                tools.push(Box::new(FindCommandTool));
            }
            // list_current_directory will be added here when implemented
        }

        // Level 2 tools (command_help, list_any_directory, pwd)
        // Level 3 tools (get_env, list_processes)

        Self { tools }
    }

    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.iter().map(|t| t.definition()).collect()
    }

    pub async fn execute(
        &self,
        name: &str,
        args: serde_json::Value,
        logger: Option<&CmdifyLogger>,
        spinner: Option<&SpinnerPause>,
    ) -> Result<ToolOutput> {
        self.tools
            .iter()
            .find(|t| t.name() == name)
            .ok_or_else(|| Error::ToolError(format!("unknown tool: {}", name)))?
            .execute(args, logger, spinner)
            .await
    }
}
```

When the registry is empty, `definitions()` returns `[]` and providers receive no tools in their request.

---

## 4. Tool Definitions (JSON Schema)

Each tool definition is authored once in `ToolDefinition` format and translated into provider-specific wire formats by each provider at request time (see [PROVIDERS.md](./PROVIDERS.md)).

### 4.1 `ask_user` (Level 1)

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
                            "description": "A single-letter key (e.g., 'A', 'B', 'Y', 'N')."
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

### 4.2 `find_command` (Level 1)

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

### 4.3 Planned Tools (Not Yet Implemented)

This is the master list of all tools planned for cmdify. Each tool is assigned a level (see §2.1) and has a brief description. Full specifications follow.

#### Master Tool List

| Tool | Level | Status | Summary |
|------|-------|--------|---------|
| `ask_user` | 1 | Implemented | Ask the user a clarifying question with multiple-choice answers |
| `find_command` | 1 | Implemented | Check whether a command exists on the system |
| `list_current_directory` | 1 | Planned | List files in the current working directory |
| `command_help` | 2 | Planned | Show help text for a command with optional grep filter |
| `list_any_directory` | 2 | Planned | List files in any user-specified directory |
| `pwd` | 2 | Planned | Print the current working directory |
| `get_env` | 3 | Planned | Read allowlisted environment variables |
| `list_processes` | 3 | Planned | List running processes |

#### `list_current_directory` (Level 1)

Lists files in the current working directory. Fixed scope — no path parameter. Uses `ls` internally with output truncation. Shares implementation with `list_any_directory`.

#### `command_help` (Level 2)

Runs `<cmd> --help` and returns the output, optionally filtered by a grep pattern. Falls back to `man <cmd>` (piped through `col -b`) if `--help` produces no useful output. The primary value is correcting the model's knowledge of command flags for the exact versions installed.

**Parameters:**

| Parameter | Required | Description |
|-----------|----------|-------------|
| `command` | yes | The command name to query (e.g., `"tar"`, `"ffmpeg"`) |
| `pattern` | no | A grep pattern to filter relevant lines (e.g., `"create"`, `"x264"`) |

**Behavior:**

- When `pattern` is provided: `<cmd> --help 2>&1 \| grep -i <pattern>`, falling back to `man <cmd> 2>&1 \| col -b \| grep -i <pattern>` if the grep result is empty
- When `pattern` is omitted: raw `<cmd> --help 2>&1`, falling back to `man <cmd> 2>&1 \| col -b`
- Hard truncation cap of 80 lines as a safety net regardless of filtering
- Timeout: 10 seconds

This approach lets the model target specific help sections rather than receiving the full (often lengthy) help text. For example, `command_help("tar", "create")` returns only lines mentioning "create", giving the model the exact `-c` / `--create` flags without noise.

#### `list_any_directory` (Level 2)

Lists files in any user-specified directory. Accepts a `path` parameter. Shares implementation with `list_current_directory`. Scope restricted to listing only (no recursive traversal, no file content reading).

#### `pwd` (Level 2)

Returns the current working directory via `std::env::current_dir()`. Simple, no subprocess needed.

#### `get_env` (Level 3)

Reads specific environment variables. Should be allowlisted (e.g., `$HOME`, `$PATH`, `$SHELL`, `$PWD`, `$USER`) rather than exposing arbitrary vars, to prevent accidental secret leakage.

#### `list_processes` (Level 3)

Runs `ps aux` and returns a truncated list of running processes. Useful for "what's running" type requests.

---

## 5. Tool Execution

### 5.1 `ask_user` (`tools/ask_user.rs`)

```
1. Pause spinner
2. Clear spinner line from stderr
3. Receive (question: "Use fd or find?", choices: [{ key: "A", label: "use fd" }, { key: "B", label: "use find" }])
4. Print to stderr:
      > [cmdify] Use fd or find?
        A) use fd
        B) use find
      > Your choice:
5. Read a single line from stdin (blocking, with 60s timeout)
6. Resume spinner
7. Trim whitespace, return as ToolOutput { content: "A" }
```

**Behavioral details:**

- **Spinner interaction**: The spinner is paused before the prompt is displayed and resumed after input is received. This prevents the spinner from overwriting the question text and keeps the display clean.
- **Timeout**: 60 seconds. If no input is received, return `ToolOutput { content: "(no response)" }` so the model can proceed or ask again.
- **Invalid input**: If the user's input doesn't match any choice key, return the raw input as-is with a note: `ToolOutput { content: "X (not a valid choice)" }`. The model decides how to handle it.
- **Output channel**: All user-facing prompts and interactive text go to **stderr**, keeping stdout clean for the final command output. This ensures pipe usage like `$(cmdify find files)` works correctly.
- **Stdin blocking**: Reading stdin is done in a blocking manner via `tokio::task::spawn_blocking`. The pure reading logic is extracted into `read_user_choice<R: BufRead>(reader, valid_keys)` for testability with `Cursor`.
- **Display format**: Each choice is displayed as `{key}) {label}`, where `key` is the single-letter identifier and `label` is the model-provided descriptive text.

### 5.2 `find_command` (`tools/find_command.rs`)

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

## 6. Provider-Specific Tool Serialization

Tool definitions and results must be translated into each provider's wire format. This translation happens inside each provider implementation (see [PROVIDERS.md — Tool Translation Layer](./PROVIDERS.md#4-tool-translation-layer)).

### Quick reference

| Provider | Tool def key | Tool call in response | Tool result sent back |
|----------|-------------|----------------------|----------------------|
| OpenAI Completions | `tools[].function` | `tool_calls[].function` | `messages[]` with `role: "tool"` |
| Anthropic | `tools[]` | `content[].type: "tool_use"` | `messages[]` with `type: "tool_result"` |
| Gemini | `tools[].function_declarations[]` | `parts[].function_call` | `contents[].parts[].function_response` |
| Responses | `tools[]` (flat) | `output[].type: "function_call"` | `input[].type: "function_call_output"` |

---

## 7. Tool Call Loop

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
            result = tool_registry.execute(
                tool_call.name,
                tool_call.arguments,
                logger,
                spinner,
            ).await
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
- **Spinner passthrough**: The spinner pause handle is passed through the tool execution chain so tools like `ask_user` can pause the spinner during interactive prompts.

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
