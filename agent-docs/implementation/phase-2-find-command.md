# Phase 2 — `find_command` Tool

## Goal

Add the tool system and the `find_command` tool, enabling the model to verify that suggested commands exist on the user's system before outputting them.

## Scope

- `Tool` trait and `ToolDefinition` / `ToolOutput` types
- `ToolRegistry` with CLI flag filtering
- `find_command` tool implementation
- Tool call loop in the orchestrator
- Completions provider: send tool definitions, parse tool calls, handle tool results
- `-n` / `--no-tools` flag wired up (no tools registered)
- `-b` / `--blind` flag wired up (disables `find_command`)

## Files to Create / Modify

```
src/
├── orchestrator.rs        # MODIFY: add tool call loop
└── tools/
    ├── mod.rs             # CREATE: Tool trait, ToolDefinition, ToolOutput, ToolRegistry
    └── find_command.rs    # CREATE: find_command tool
tests/
├── tools_test.rs          # CREATE: tool execution tests
└── orchestrator_test.rs   # CREATE: end-to-end tests with mock provider
```

## Implementation Steps

### 2.1 Tool types (`src/tools/mod.rs`)

Define core abstractions:

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

### 2.2 Tool registry (`src/tools/mod.rs`)

```rust
pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new(cli_flags: &CliFlags) -> Self { ... }
    pub fn definitions(&self) -> Vec<ToolDefinition> { ... }
    pub async fn execute(&self, name: &str, args: serde_json::Value) -> Result<ToolOutput> { ... }
}
```

Flag filtering:

| Flag | Tools registered |
|------|-----------------|
| (none) | (empty — `ask_user` not yet implemented) |
| `-b` | (empty — `ask_user` not yet implemented) |
| `-n` | (empty — no tools) |

In this phase, the registry only ever contains `find_command` (or is empty). The `-q` flag has no effect yet since `ask_user` doesn't exist.

### 2.3 `find_command` tool (`src/tools/find_command.rs`)

Per `TOOLS.md §4.2`:

- Runs `sh -c "command -v $1" -- {command}` as a subprocess
- Falls back to `which {command}` if the `sh` command fails
- 5-second timeout via `tokio::time::timeout`
- Returns `ToolOutput { content: "/path/to/cmd" }` on success
- Returns `ToolOutput { content: "not found" }` if not found
- Returns `ToolOutput { content: "error: command lookup timed out" }` on timeout
- Shell injection safety: command name passed as argument, not interpolated

### 2.4 Completions provider: tool support (`src/provider/completions.rs`)

Update the completions provider to:

1. **Send tool definitions** in the request body under `tools[].function` format
2. **Parse tool calls** from `choices[0].message.tool_calls[]`
3. **Format tool results** as `messages[]` with `role: "tool"`, `tool_call_id`, and `content`
4. **Map `finish_reason: "tool_calls"`** to `FinishReason::ToolCalls`

Wire format per `PROVIDERS.md §3.1`.

### 2.5 Tool call loop (`src/orchestrator.rs`)

Replace single-shot flow with the tool call loop per `TOOLS.md §6`:

```
messages = [Message::System, Message::User]

loop (max 10 iterations):
    response = provider.send_request(&messages, &tool_definitions)

    if response has content AND no tool_calls:
        print content to stdout, exit 0

    if response has tool_calls:
        append Message::Assistant { content, tool_calls } to messages
        for each tool_call:
            result = registry.execute(tool_call.name, tool_call.args).await
            append Message::ToolResult { ... } to messages
        continue loop

    error: empty response, exit 1
```

### 2.6 System prompt update

Update `src/system_prompt.txt` to mention the `find_command` tool and instruct the model to use it to verify commands exist before suggesting them.

## Tests

**Unit tests:**
- `tools/mod.rs`: registry creation, filtering by flags, execute unknown tool returns error
- `tools/find_command.rs`: successful lookup, not found, timeout, shell injection safety
- `provider/completions.rs`: tool definition formatting, tool call parsing, tool result message formatting, finish_reason mapping

**Integration tests:**
- `tools_test.rs`: real subprocess `command -v` for common commands (ls, sh, cat)
- `orchestrator_test.rs`: mock provider returns a tool call → tool executed → mock provider returns final answer → printed to stdout

## Acceptance Criteria

- [ ] Model can call `find_command` to verify commands exist on the system
- [ ] `-b` / `--blind` disables `find_command`
- [ ] `-n` / `--no-tools` disables all tools (single-shot behavior)
- [ ] Tool call loop executes correctly (tool call → result → next request)
- [ ] Loop exits after max 10 iterations
- [ ] `make check` passes
