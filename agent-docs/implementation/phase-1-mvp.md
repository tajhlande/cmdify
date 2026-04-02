# Phase 1 — Minimal MVP

## Goal

Get a working binary that sends a natural language request to any OpenAI-compatible `/completions` endpoint and prints the resulting shell command. No tools. Minimal but functional UX.

## Scope

- Project scaffolding (Cargo.toml, module structure)
- CLI argument parsing with `clap`
- Environment variable configuration (`CMDIFY_PROVIDER_NAME`, `CMDIFY_MODEL_NAME`, `CMDIFY_COMPLETIONS_URL`, `CMDIFY_COMPLETIONS_KEY`)
- System prompt (embedded at compile time via `include_str!`)
- Shell detection (`$SHELL` env var, appended to system prompt)
- Single provider: `completions` (generic OpenAI-compatible `/v1/chat/completions`)
- Single-shot request: send messages, print response, exit
- No tools, no tool call loop
- Basic `Makefile` (`build`, `dev`, `test`, `lint`, `fmt`, `check`, `clean`, `install`)
- Error handling (`thiserror` Error enum)
- System prompt runtime override via `CMDIFY_SYSTEM_PROMPT` env var

## Files to Create

```
Cargo.toml
Makefile
src/
├── main.rs
├── cli.rs
├── config.rs
├── error.rs
├── orchestrator.rs
├── prompt.rs
├── system_prompt.txt
└── provider/
    ├── mod.rs
    └── completions.rs
tests/
├── config_test.rs
├── cli_test.rs
└── provider_test.rs
```

## Dependencies

```toml
[dependencies]
clap = { version = "4", features = ["derive"] }
reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"

[dev-dependencies]
wiremock = "0.6"
```

## Implementation Steps

### 1.1 Project scaffolding

- Initialize `Cargo.toml` with dependencies
- Create `Makefile` with `build`, `dev`, `test`, `lint`, `fmt`, `check`, `clean`, `install` targets
- Create `src/main.rs` with `#[tokio::main]` entry point

### 1.2 Error types (`src/error.rs`)

Define the `Error` enum per `BUILD.md §3`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("config error: {0}")]
    ConfigError(String),
    #[error("provider error: {0}")]
    ProviderError(String),
    #[error("response error: unexpected format: {0}")]
    ResponseError(String),
    #[error("http error: {0}")]
    HttpError(#[from] reqwest::Error),
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),
}
```

Type alias `Result<T> = std::result::Result<T, Error>`.

### 1.3 CLI (`src/cli.rs`)

- `clap` derive struct with `#[command(name = "cmdify", about, version)]`
- Positional args: `prompt: Vec<String>` (joined with spaces)
- Flags: `--quiet` / `-q`, `--blind` / `-b`, `--no-tools` / `-n` (parsed but unused until later phases)
- If no positional args, print help and exit 0

### 1.4 Configuration (`src/config.rs`)

- `Config` struct: `provider_name`, `model_name`, `max_tokens`, `system_prompt_override`
- `ProviderSettings` struct: `api_key` (Option<String> — not required), `base_url` (String)
- `AuthStyle` enum: `Header { name, prefix }`, `QueryParam { name }`
- `Config::from_env()`:
  - Read `CMDIFY_PROVIDER_NAME` (required)
  - Read `CMDIFY_MODEL_NAME` (required)
  - Read `CMDIFY_MAX_TOKENS` (optional, default 4096)
  - Read `CMDIFY_SYSTEM_PROMPT` (optional path to file)
  - Load provider settings based on provider name
- For the `completions` provider:
  - `CMDIFY_COMPLETIONS_URL` required
  - `CMDIFY_COMPLETIONS_KEY` optional (some local models need no auth)
- **API key is optional**: if not set, requests are sent without an auth header

### 1.5 Shared types (`src/provider/mod.rs`)

Define the shared types that all providers will use:

```rust
pub enum Message {
    System { content: String },
    User { content: String },
    Assistant { content: Option<String>, tool_calls: Vec<ToolCall> },
    ToolResult { tool_call_id: String, name: String, content: String },
}

pub struct ProviderResponse {
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub finish_reason: FinishReason,
}

pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

pub enum FinishReason {
    Stop,
    ToolCalls,
    Length,
    Other(String),
}
```

Define the `Provider` trait:

```rust
#[async_trait]
pub trait Provider: Send + Sync {
    async fn send_request(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<ProviderResponse>;
    fn supports_tools(&self) -> bool;
    fn name(&self) -> &str;
}
```

Factory function `create_provider(config: &Config) -> Result<Box<dyn Provider>>` — initially only matches `"completions"`.

### 1.6 Completions provider (`src/provider/completions.rs`)

Implements `Provider` for OpenAI-compatible `/v1/chat/completions`.

- Constructs request body with `model`, `messages` (no `tools` in this phase), and optional `max_tokens`
- Sends POST to `{base_url}/v1/chat/completions`
- If `api_key` is set, adds `Authorization: Bearer {key}` header; otherwise, sends no auth header
- Parses response:
  - Extract `choices[0].message.content` as `ProviderResponse.content`
  - `choices[0].finish_reason` mapped to `FinishReason`
  - `tool_calls` parsed if present (returns empty vec in this phase)
- Error on non-200 status codes, extract error message from response body

### 1.7 System prompt (`src/prompt.rs` + `src/system_prompt.txt`)

- `src/system_prompt.txt`: instruct the model to return a single shell command compatible with bash and zsh, no markdown fences, no explanation
- `src/prompt.rs`:
  - `pub const EMBEDDED_SYSTEM_PROMPT: &str = include_str!("system_prompt.txt");`
  - `pub fn load_system_prompt(config: &Config) -> Result<String>`:
    - If `CMDIFY_SYSTEM_PROMPT` is set, read file at that path
    - Otherwise, use `EMBEDDED_SYSTEM_PROMPT`
    - Append shell detection: detect `$SHELL`, append `"The user's shell is {shell}."`

### 1.8 Orchestrator (`src/orchestrator.rs`)

Single-shot flow (no tool loop yet):

```
1. Assemble user prompt from CLI args
2. Load system prompt (from config override or embedded)
3. Build messages: [Message::System, Message::User]
4. Create provider from config
5. Call provider.send_request(&messages, &[])
6. If response has content, print to stdout
7. If response has tool_calls, print error (tools not yet supported)
8. Exit 0 on success, 1 on error
```

### 1.9 Tests

**Unit tests** (in-source `#[cfg(test)]`):
- `cli.rs`: parse flags, verify help output, verify positional args joined
- `config.rs`: missing required vars, optional vars, completions provider settings
- `provider/completions.rs`: request body formatting, response parsing with sample JSON
- `prompt.rs`: shell detection, embedded prompt loading

**Integration tests** (`tests/`):
- `config_test.rs`: env var loading, error messages
- `cli_test.rs`: clap `CommandFactory` parsing tests
- `provider_test.rs`: mock HTTP server with `wiremock`, verify correct request sent, response parsed

## Acceptance Criteria

- [ ] `cargo build --release` produces a static binary with no external runtime dependencies
- [ ] `CMDIFY_PROVIDER_NAME=completions CMDIFY_MODEL_NAME=llama3 CMDIFY_COMPLETIONS_URL=http://localhost:11434 cmdify list all files` sends a request to the local endpoint and prints a command
- [ ] `CMDIFY_PROVIDER_NAME=completions CMDIFY_MODEL_NAME=llama3 CMDIFY_COMPLETIONS_URL=http://localhost:11434 cmdify` (no args) prints help and exits 0
- [ ] No API key required — works against local models with no auth
- [ ] API key used when provided via `CMDIFY_COMPLETIONS_KEY`
- [ ] `make check` passes (clippy + fmt + test)
- [ ] Shell detected and included in system prompt
- [ ] `CMDIFY_SYSTEM_PROMPT` env var overrides compiled-in prompt
