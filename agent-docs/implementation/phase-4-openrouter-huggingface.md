# Phase 4 — OpenRouter & HuggingFace Providers

## Goal

Add two named providers that use the OpenAI Completions wire format. This phase establishes the "named provider" pattern: a thin wrapper that sets default base URL and auth header, then delegates to the shared completions implementation.

## Scope

- Refactor `completions.rs` into a shared module (common request/response logic)
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

### 4.1 Refactor shared completions logic

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

### 4.2 OpenRouter provider (`src/provider/openrouter.rs`)

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

### 4.3 HuggingFace provider (`src/provider/huggingface.rs`)

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

- Default base URL: `https://api-inference.huggingface.co`
- Auth header: `Authorization: Bearer {HUGGINGFACE_API_KEY}`
- Delegates to `send_completions_request()`

### 4.4 Config and factory updates

- `config.rs`: add match arms for `"openrouter"` and `"huggingface"` in `ProviderSettings::from_env()`
- `provider/mod.rs`: add match arms in `create_provider()`

### 4.5 API key required for named providers

Unlike the generic `completions` provider (where the key is optional for local models), named providers like OpenRouter and HuggingFace require their API key. The `ProviderSettings::header()` constructor should error if the key env var is missing.

## Tests

**Unit tests:**
- `provider/openrouter.rs`: correct default URL, correct auth header format
- `provider/huggingface.rs`: correct default URL, correct auth header format
- Wire format: both providers produce identical request body structure to completions provider

**Integration tests:**
- `provider_test.rs`: mock HTTP server, verify correct URL path and headers for each provider

## Acceptance Criteria

- [ ] `CMDIFY_PROVIDER_NAME=openrouter CMDIFY_MODEL_NAME=... OPENROUTER_API_KEY=... cmdify find files` works
- [ ] `CMDIFY_PROVIDER_NAME=huggingface CMDIFY_MODEL_NAME=... HUGGINGFACE_API_KEY=... cmdify find files` works
- [ ] Both providers support tools (find_command, ask_user) via shared completions logic
- [ ] Missing API key produces clear error message
- [ ] Base URL can be overridden via `OPENROUTER_BASE_URL` / `HUGGINGFACE_BASE_URL`
- [ ] `make check` passes
