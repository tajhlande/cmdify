# Phase 6 — OpenRouter & HuggingFace Providers

## Goal

Add two named providers that use the OpenAI Completions wire format. This phase establishes the "named provider" pattern: a thin wrapper that sets default base URL and auth header, then delegates to the shared completions implementation.

## Scope

- Refactor `completions.rs` into a shared module (common request/response logic)
- Set `User-Agent` header on all provider HTTP requests: `cmdify/<version>` (e.g. `cmdify/0.4.0`)
- Create `OpenRouterProvider` as a thin struct wrapping shared completions logic
- Create `HuggingFaceProvider` as a thin struct wrapping shared completions logic
- Update factory function
- Environment variables: `OPENROUTER_API_KEY`, `OPENROUTER_BASE_URL`, `HUGGINGFACE_API_KEY`, `HUGGINGFACE_BASE_URL`

## Files to Create / Modify

```
src/
├── config.rs              # MODIFY: add OpenRouter and HuggingFace provider settings
└── provider/
    ├── mod.rs             # MODIFY: factory, add new providers
    ├── completions.rs     # REFACTOR: extract shared logic into internal functions
    ├── openrouter.rs      # CREATE: thin OpenRouter wrapper
    └── huggingface.rs     # CREATE: thin HuggingFace wrapper
tests/
└── provider_test.rs       # MODIFY: add tests for new providers
```

## Implementation Steps

### 6.1 User-Agent header

All provider HTTP clients must set a `User-Agent` header identifying cmdify and its version. The version should come from the `CARGO_PKG_VERSION` env variable (set at compile time by cargo) or the `clap::Parser` version accessor.

```rust
// In shared completions request builder (and any future provider HTTP clients)
let user_agent = format!("cmdify/{}", env!("CARGO_PKG_VERSION"));
request_builder = request_builder.header("User-Agent", &user_agent);
```

This applies to all providers, not just OpenRouter and HuggingFace. Since this phase refactors completions into shared logic, it's the natural place to add it. Existing providers (`completions`, `responses`) should also be updated.

### 6.2 Refactor shared completions logic

Extract the core of `completions.rs` into reusable internal functions:

```rust
// In completions.rs or a new internal module
pub(crate) async fn send_completions_request(
    client: &reqwest::Client,
    base_url: &str,
    auth_style: &Option<AuthHeader>,
    model: &str,
    messages: &[Message],
    tools: &[ToolDefinition],
    max_tokens: u32,
) -> Result<ProviderResponse> { ... }

pub(crate) fn format_completions_messages(messages: &[Message]) -> serde_json::Value { ... }
pub(crate) fn format_completions_tools(tools: &[ToolDefinition]) -> serde_json::Value { ... }
pub(crate) fn parse_completions_response(body: &serde_json::Value) -> Result<ProviderResponse> { ... }
```

### 6.3 OpenRouter provider (`src/provider/openrouter.rs`)

Thin struct:

```rust
pub struct OpenRouterProvider {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
    max_tokens: u32,
}
```

- Default base URL: `https://openrouter.ai/api`
- Auth header: `Authorization: Bearer {OPENROUTER_API_KEY}`
- Delegates to `send_completions_request()`
- Additional header: `HTTP-Referer` for OpenRouter's ranking (optional, can add later)

### 6.4 HuggingFace provider (`src/provider/huggingface.rs`)

Thin struct, same pattern:

```rust
pub struct HuggingFaceProvider {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
    max_tokens: u32,
}
```

- Default base URL: `https://router.huggingface.co/v1`
- Auth header: `Authorization: Bearer {HUGGINGFACE_API_KEY}`
- Delegates to `send_completions_request()`

### 6.5 Config and factory updates

- `config.rs`: add match arms for `"openrouter"` and `"huggingface"` in `ProviderSettings::from_env()`
- `provider/mod.rs`: add match arms in `create_provider()`

### 6.6 API key required for named providers

Unlike the generic `completions` provider (where the key is optional for local models), named providers like OpenRouter and HuggingFace require their API key. The `ProviderSettings::header()` constructor should error if the key env var is missing.

### 6.7 Live integration test scripts (`examples/`)

Create an `examples/` directory containing shell scripts that exercise each provider against a real API. These scripts are for manual validation only — they are NOT run by CI or `make check`. Each script:

- Sets the required environment variables (`CMDIFY_PROVIDER_NAME`, `CMDIFY_MODEL_NAME`, provider-specific keys)
- Invokes the `cmdify` binary with a simple prompt
- Documents expected output and any prerequisites (API keys, model availability)
- Is executable (`chmod +x`) with a standard shebang line

Initial scripts:

```
examples/
├── test-openrouter.sh      # OpenRouter end-to-end (requires OPENROUTER_API_KEY)
├── test-huggingface.sh     # HuggingFace end-to-end (requires HUGGINGFACE_API_KEY)
├── test-completions.sh     # Generic completions provider (requires CMDIFY_COMPLETIONS_KEY)
└── README.md               # Instructions for running, expected behavior, key setup
```

The `examples/README.md` should explain that these scripts hit live APIs, may incur costs, and are intended for developer use during integration testing — not automated CI.

> **Note:** Future phases that add new providers (Anthropic, Gemini, Qwen, Kimi, etc.) should follow the same pattern and add a corresponding `examples/test-<provider>.sh` script.

## Tests

**Unit tests:**
- `provider/openrouter.rs`: correct default URL, correct auth header format
- `provider/huggingface.rs`: correct default URL, correct auth header format
- Wire format: both providers produce identical request body structure to completions provider

**Integration tests:**
- `provider_test.rs`: mock HTTP server, verify correct URL path and headers for each provider

## Acceptance Criteria

- [x] `CMDIFY_PROVIDER_NAME=openrouter CMDIFY_MODEL_NAME=... OPENROUTER_API_KEY=... cmdify find files` works
- [x] `CMDIFY_PROVIDER_NAME=huggingface CMDIFY_MODEL_NAME=... HUGGINGFACE_API_KEY=... cmdify find files` works
- [x] Both providers support tools (find_command, ask_user) via shared completions logic
- [x] Missing API key produces clear error message
- [x] Base URL can be overridden via `OPENROUTER_BASE_URL` / `HUGGINGFACE_BASE_URL`
- [x] All provider HTTP requests include `User-Agent: cmdify/<version>` header
- [x] `examples/` directory exists with live integration test scripts for OpenRouter and HuggingFace
- [x] `examples/README.md` documents how to run scripts and expected prerequisites
- [x] `make check` passes
- [x] Hand-tested against live OpenRouter and HuggingFace APIs
