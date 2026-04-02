# cmdify — Build, Configuration & Testing

## 1. Configuration

Configuration uses a three-layer precedence: **environment variable > config file > hardcoded default**.

Non-secret settings (`provider_name`, `model_name`, `max_tokens`, `system_prompt`) can be provided in either environment variables or a config file. API keys are always read from environment variables only — they are intentionally excluded from the config file to avoid storing secrets in plaintext on disk.

Missing required settings (when neither env var nor config file provides them) produce a clear error message on stderr and exit with code 1.

### 1.0 Config File

An optional TOML file can provide default values for non-secret settings. The file is searched at:

1. `$XDG_CONFIG_HOME/cmdify/config.toml` (if `XDG_CONFIG_HOME` is set)
2. `$HOME/.config/cmdify/config.toml` (fallback)

The file is entirely optional. If it does not exist, all settings must come from environment variables or defaults.

```toml
provider_name = "openai"
model_name = "gpt-4o"
max_tokens = 4096
system_prompt = "/path/to/custom_prompt.txt"
```

Only the keys listed above are recognized. Unknown keys are silently ignored by the TOML parser.

### 1.1 Core Settings

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `CMDIFY_PROVIDER_NAME` / `provider_name` | Yes | — | Provider identifier (e.g., `openai`, `completions`, `anthropic`) |
| `CMDIFY_MODEL_NAME` / `model_name` | Yes | — | Model name to use (e.g., `gpt-4`, `claude-sonnet-4-20250514`) |
| `CMDIFY_MAX_TOKENS` / `max_tokens` | No | 4096 | Max tokens for providers that require it (e.g., Anthropic) |
| `CMDIFY_SYSTEM_PROMPT` / `system_prompt` | No | — | Path to a file containing a custom system prompt, overriding the compiled-in default |

### 1.2 Per-Provider Settings

Each provider only reads its own variables. Unrelated variables are ignored.

| Variable | Provider | Description |
|----------|----------|-------------|
| `OPENAI_API_KEY` | openai | API key (sent as `Authorization: Bearer` header) |
| `OPENAI_BASE_URL` | openai | Custom base URL (default: `https://api.openai.com`) |
| `ANTHROPIC_API_KEY` | anthropic | API key (sent as `x-api-key` header) |
| `ANTHROPIC_BASE_URL` | anthropic | Custom base URL (default: `https://api.anthropic.com`) |
| `GEMINI_API_KEY` | gemini | API key (sent as `?key=` query parameter) |
| `GEMINI_BASE_URL` | gemini | Custom base URL (default: `https://generativelanguage.googleapis.com`) |
| `CMDIFY_COMPLETIONS_KEY` | completions | API key |
| `CMDIFY_COMPLETIONS_URL` | completions | API URL (required — no default) |
| `CMDIFY_RESPONSES_KEY` | responses | API key |
| `CMDIFY_RESPONSES_URL` | responses | API URL (required — no default) |
| `ZAI_API_KEY` | zai | API key |
| `ZAI_BASE_URL` | zai | Custom base URL (default: `https://api.z.ai`) |
| `MINIMAX_API_KEY` | minimax | API key |
| `MINIMAX_BASE_URL` | minimax | Custom base URL (default: `https://api.minimax.chat`) |
| `QWEN_API_KEY` | qwen | API key |
| `QWEN_BASE_URL` | qwen | Custom base URL (default: `https://dashscope.aliyuncs.com/compatible-mode`) |
| `KIMI_API_KEY` | kimi | API key |
| `KIMI_BASE_URL` | kimi | Custom base URL (default: `https://api.moonshot.cn`) |
| `MISTRAL_API_KEY` | mistral | API key |
| `MISTRAL_BASE_URL` | mistral | Custom base URL (default: `https://api.mistral.ai`) |
| `OPENROUTER_API_KEY` | openrouter | API key |
| `OPENROUTER_BASE_URL` | openrouter | Custom base URL (default: `https://openrouter.ai/api`) |
| `HUGGINGFACE_API_KEY` | huggingface | API key |
| `HUGGINGFACE_BASE_URL` | huggingface | Custom base URL (default: `https://api-inference.huggingface.co`) |
| `OLLAMA_BASE_URL` | ollama | Custom base URL (default: `http://localhost:11434`) — no API key required |

### 1.3 Config Struct

Configuration is loaded in three layers (env var > config file > default), then provider-specific settings based on `CMDIFY_PROVIDER_NAME`. This avoids eagerly loading credentials for unused providers.

**FileConfig** (internal, deserialized from TOML):

```rust
#[derive(Deserialize)]
struct FileConfig {
    provider_name: Option<String>,
    model_name: Option<String>,
    max_tokens: Option<u32>,
    system_prompt: Option<String>,
}
```

```rust
pub enum AuthStyle {
    Header { name: String, prefix: String },
    QueryParam { name: String },
}

pub struct ProviderSettings {
    pub api_key: String,
    pub base_url: String,
    pub auth_style: AuthStyle,
}
```

Most providers use `AuthStyle::Header { name: "Authorization", prefix: "Bearer " }`. Anthropic uses `AuthStyle::Header { name: "x-api-key", prefix: "" }`. Gemini uses `AuthStyle::QueryParam { name: "key" }`. Each provider reads the key and applies it according to its `auth_style`.

```rust
impl Config {
    pub fn from_env() -> Result<Self> {
        let file_config = config_file_path()
            .map(|p| load_file_config(&p))
            .transpose()?;

        let provider_name = env::var("CMDIFY_PROVIDER_NAME").ok()
            .or_else(|| file_config.as_ref().and_then(|f| f.provider_name.clone()))
            .ok_or_else(|| Error::ConfigError(
                "CMDIFY_PROVIDER_NAME is required (set env var or provider_name in config file)".into(),
            ))?;

        let model_name = env::var("CMDIFY_MODEL_NAME").ok()
            .or_else(|| file_config.as_ref().and_then(|f| f.model_name.clone()))
            .ok_or_else(|| Error::ConfigError(
                "CMDIFY_MODEL_NAME is required (set env var or model_name in config file)".into(),
            ))?;

        let max_tokens = env::var("CMDIFY_MAX_TOKENS")
            .ok()
            .and_then(|v| v.parse().ok())
            .or_else(|| file_config.as_ref().and_then(|f| f.max_tokens))
            .unwrap_or(4096);

        let system_prompt_override = env::var("CMDIFY_SYSTEM_PROMPT").ok()
            .or_else(|| file_config.as_ref().and_then(|f| f.system_prompt.clone()));

        let provider_settings = ProviderSettings::from_env(&provider_name)?;
        Ok(Self { provider_name, model_name, max_tokens, system_prompt_override, provider_settings })
    }
}
```

Most providers use `AuthStyle::Header { name: "Authorization", prefix: "Bearer " }`. Anthropic uses `AuthStyle::Header { name: "x-api-key", prefix: "" }`. Gemini uses `AuthStyle::QueryParam { name: "key" }`. Each provider reads the key and applies it according to its `auth_style`.

```rust
impl Config {
    pub fn from_env() -> Result<Self> {
        let provider_name = env::var("CMDIFY_PROVIDER_NAME")
            .map_err(|_| Error::ConfigError("CMDIFY_PROVIDER_NAME is required".into()))?;
        let model_name = env::var("CMDIFY_MODEL_NAME")
            .map_err(|_| Error::ConfigError("CMDIFY_MODEL_NAME is required".into()))?;
        let max_tokens = env::var("CMDIFY_MAX_TOKENS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(4096);
        let provider_settings = ProviderSettings::from_env(&provider_name)?;
        Ok(Self { provider_name, model_name, max_tokens, provider_settings })
    }
}

impl ProviderSettings {
    fn from_env(provider_name: &str) -> Result<Self> {
        match provider_name {
            "openai" => Self::header("OPENAI_API_KEY", "OPENAI_BASE_URL", "https://api.openai.com", "Authorization", "Bearer "),
            "anthropic" => Self::header("ANTHROPIC_API_KEY", "ANTHROPIC_BASE_URL", "https://api.anthropic.com", "x-api-key", ""),
            "gemini" => Self::query_param("GEMINI_API_KEY", "GEMINI_BASE_URL", "https://generativelanguage.googleapis.com", "key"),
            "completions" => Self::header_required_url("CMDIFY_COMPLETIONS_KEY", "CMDIFY_COMPLETIONS_URL", "Authorization", "Bearer "),
            "responses" => Self::header_required_url("CMDIFY_RESPONSES_KEY", "CMDIFY_RESPONSES_URL", "Authorization", "Bearer "),
            // ... each named provider with its own key/base URL/auth defaults
            other => Err(Error::ConfigError(format!("unknown provider: {}", other))),
        }
    }

    fn header(key_var: &str, url_var: &str, default_url: &str, header_name: &str, header_prefix: &str) -> Result<Self> {
        let api_key = env::var(key_var)
            .map_err(|_| Error::ConfigError(format!("{} is required for this provider", key_var)))?;
        let base_url = env::var(url_var).unwrap_or_else(|_| default_url.into());
        Ok(Self { api_key, base_url, auth_style: AuthStyle::Header { name: header_name.into(), prefix: header_prefix.into() } })
    }

    fn query_param(key_var: &str, url_var: &str, default_url: &str, param_name: &str) -> Result<Self> {
        let api_key = env::var(key_var)
            .map_err(|_| Error::ConfigError(format!("{} is required for this provider", key_var)))?;
        let base_url = env::var(url_var).unwrap_or_else(|_| default_url.into());
        Ok(Self { api_key, base_url, auth_style: AuthStyle::QueryParam { name: param_name.into() } })
    }

    fn header_required_url(key_var: &str, url_var: &str, header_name: &str, header_prefix: &str) -> Result<Self> {
        let api_key = env::var(key_var)
            .map_err(|_| Error::ConfigError(format!("{} is required", key_var)))?;
        let base_url = env::var(url_var)
            .map_err(|_| Error::ConfigError(format!("{} is required", url_var)))?;
        Ok(Self { api_key, base_url, auth_style: AuthStyle::Header { name: header_name.into(), prefix: header_prefix.into() } })
    }
}
```

Only the selected provider's credentials are loaded. Misconfigured keys for other providers produce no warnings or errors. Adding a new provider requires only a new match arm in `ProviderSettings::from_env`.

---

## 2. Dependencies

| Crate | Purpose | Notes |
|-------|---------|-------|
| `clap` | CLI argument parsing | derive API, with `string` feature |
| `reqwest` | HTTP client | `rustls-tls` feature (no native TLS) |
| `tokio` | Async runtime | `rt-multi-thread`, `macros`, `process`, `io-util`, `time` |
| `serde` | Serialization | `derive` feature |
| `serde_json` | JSON handling | |
| `toml` | Config file parsing | TOML deserialization for `FileConfig` |
| `async-trait` | Async trait support | for Provider and Tool traits |
| `thiserror` | Error enum derivation | |

**Static binary requirement:** `rustls` is used instead of `native-tls` for TLS to avoid linking to system OpenSSL. This ensures the binary is self-contained with no external runtime dependencies.

---

## 3. Error Handling

A custom `Error` enum using `thiserror`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("config error: {0}")]
    ConfigError(String),

    #[error("provider error: {0}")]
    ProviderError(String),

    #[error("tool error: {0}")]
    ToolError(String),

    #[error("response error: unexpected format: {0}")]
    ResponseError(String),

    #[error("http error: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),
}
```

All errors are printed to stderr. The binary exits with code 1 on error, 0 on success.

---

## 4. Build Process

No `build.rs` script is needed. The system prompt is embedded via `include_str!()` with no special build step. A top-level `Makefile` provides project-level build, test, lint, and distribution targets.

### 4.1 `Makefile` Targets

| Target | Description |
|--------|-------------|
| `make build` | `cargo build --release` — native release binary |
| `make dev` | `cargo build` — debug binary |
| `make test` | `cargo test` — run all unit and integration tests |
| `make lint` | `cargo clippy -- -D warnings && cargo fmt --check` |
| `make fmt` | `cargo fmt` — auto-format code |
| `make check` | `make lint && make test` — full pre-commit check |
| `make clean` | `cargo clean` |
| `make dist` | Build release binaries for all supported targets |
| `make install` | `cargo install --path .` — install to `~/.cargo/bin` |

### 4.2 Cross-Compilation (`make dist`)

Builds static binaries for supported platforms:

| Target | Platform |
|--------|----------|
| `x86_64-apple-darwin` | macOS Intel |
| `aarch64-apple-darwin` | macOS Apple Silicon |
| `x86_64-unknown-linux-musl` | Linux x86_64 (static) |
| `aarch64-unknown-linux-musl` | Linux ARM64 (static) |

Musl targets require the `rust-lld` linker and may need `x86_64-unknown-linux-musl` / `aarch64-unknown-linux-musl` toolchains installed via `rustup`.

Output binaries are placed in `target/dist/<target_name>/cmdify`.

### 4.3 No Build Script

`build.rs` is intentionally omitted. There is no codegen, no C dependency linking, and no conditional compilation that requires it. The `include_str!()` macro handles prompt embedding natively within the compiler.

---

## 5. Testing Strategy

### 5.1 Unit Tests

Live in each source file inside `#[cfg(test)] mod tests { ... }` blocks.

| Layer | Approach |
|-------|----------|
| `config` | Test with env vars set/unset; verify error messages for missing required vars; test TOML config file parsing, precedence (env > file > default), and XDG/HOME path resolution |
| `cli` | Test via `clap`'s `CommandFactory` testing utilities; verify flag parsing and help output |
| `tools/` | Test `find_command` with mocked subprocess; test `ask_user` with mocked stdin |
| `provider/` | Test wire format serialization (format_tools, parse_tool_calls) with sample JSON |

### 5.2 Integration Tests

Live in `tests/` at the project root.

| Test file | Approach |
|-----------|----------|
| `config_test.rs` | Integration tests for config loading with env vars and config file precedence |
| `provider_test.rs` | Integration tests with a mock HTTP server (e.g., `wiremock` or `mockito`) |
| `tools_test.rs` | Integration tests for tool execution with real subprocesses where safe |
| `orchestrator_test.rs` | End-to-end tests with a mock provider that returns canned responses |

### 5.3 CI

A GitHub Actions workflow at `.github/workflows/ci.yml` should run:

```
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```

The workflow should trigger on pushes to `main` and on pull requests.

---

## 6. Distribution

- Single binary output via `cargo build --release`
- Target-specific builds: `cargo build --release --target <target>`
- No runtime dependencies — static binary via `rustls` and musl target on Linux
- Optional: GitHub Actions for cross-compilation and release binaries
