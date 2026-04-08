# Phase 8 — Responses Provider & Remaining Providers

## Goal

Complete the full provider lineup by adding the OpenAI Responses API provider, the remaining named providers (Z.ai, Minimax, Qwen, Kimi, Mistral), and Ollama.

## Scope

- **Responses provider** (`responses`): OpenAI Responses API wire format (distinct from Completions)
- **Z.ai** (`zai`): thin wrapper around completions
- **Minimax** (`minimax`): thin wrapper around completions
- **Qwen** (`qwen`): thin wrapper around completions
- **Kimi** (`kimi`): thin wrapper around completions
- **Mistral** (`mistral`): thin wrapper around completions
- **Ollama** (`ollama`): thin wrapper around completions, default `http://localhost:11434`, no auth required

## Files to Create / Modify

```
src/
├── config.rs              # MODIFY: add all new provider settings
└── provider/
    ├── mod.rs             # MODIFY: factory with all providers
    ├── responses.rs       # CREATE: full Responses API implementation
    ├── zai.rs             # CREATE: thin completions wrapper
    ├── minimax.rs         # CREATE: thin completions wrapper
    ├── qwen.rs            # CREATE: thin completions wrapper
    ├── kimi.rs            # CREATE: thin completions wrapper
    ├── mistral.rs         # CREATE: thin completions wrapper
    └── ollama.rs          # CREATE: thin completions wrapper, no auth
tests/
└── provider_test.rs       # MODIFY: add tests
```

## Implementation Steps

### 8.1 Responses provider (`src/provider/responses.rs`)

Full custom implementation per `PROVIDERS.md §3.4`:

**Wire format differences from Completions:**
- Endpoint: `POST /v1/responses`
- Input array uses `input` (not `messages`)
- Tools use flat format: `{ type: "function", name, description, parameters }` (not nested under `function`)
- Tool calls appear as `{ type: "function_call", id, call_id, name, arguments }` in `output[]`
- Tool results sent as `{ type: "function_call_output", call_id, output }` in next request's `input[]`
- Final text appears as `{ type: "message", content: [{ type: "output_text", text }] }` in `output[]`
- Stateless mode only (full `input` sent each turn, no `previous_response_id`)

**Private methods:**
- `format_messages()`: translate `Message` variants to Responses API `input` format
- `format_tools()`: flat tool format (no `function` nesting)
- `parse_response()`: parse `output[]` for `function_call` and `message` items
- Tool results: `Message::ToolResult` → `{ type: "function_call_output", call_id, output }`

**Config:**
- `CMDIFY_RESPONSES_URL` (required, no default)
- `CMDIFY_RESPONSES_KEY` (optional)

### 8.2 Remaining completions-category providers

Each is a thin struct following the Phase 6 pattern:

| Provider | Module | Default Base URL | Key Env Var |
|----------|--------|-----------------|-------------|
| `zai` | `zai.rs` | `https://api.z.ai/api/paas/v4` | `ZAI_API_KEY` |
| `minimax` | `minimax.rs` | `https://api.minimax.io` | `MINIMAX_API_KEY` |
| `qwen` | `qwen.rs` | `https://dashscope-intl.aliyuncs.com/compatible-mode` | `QWEN_API_KEY` |
| `kimi` | `kimi.rs` | `https://api.moonshot.ai` | `KIMI_API_KEY` |
| `mistral` | `mistral.rs` | `https://api.mistral.ai` | `MISTRAL_API_KEY` |
| `ollama` | `ollama.rs` | `http://localhost:11434` | *(none — no auth)* |

All delegate to `send_completions_request()` with their respective defaults. All require their API key (except Ollama, which requires no auth). All allow base URL override via `_<PROVIDER>_BASE_URL`.

### 8.3 Config and factory updates

- `config.rs`: add match arms for all six new providers
- `provider/mod.rs`: complete the factory with all 12 providers
- Update `ProviderSettings::from_env()` to handle all cases

### 8.4 Factory completeness

Final factory:

```rust
pub fn create_provider(config: &Config) -> Result<Box<dyn Provider>> {
    match config.provider_name.as_str() {
        "openai" => Ok(Box::new(OpenAiProvider::new(config)?)),
        "anthropic" => Ok(Box::new(AnthropicProvider::new(config)?)),
        "gemini" => Ok(Box::new(GeminiProvider::new(config)?)),
        "completions" => Ok(Box::new(CompletionsProvider::new(config)?)),
        "responses" => Ok(Box::new(ResponsesProvider::new(config)?)),
        "openrouter" => Ok(Box::new(OpenRouterProvider::new(config)?)),
        "huggingface" => Ok(Box::new(HuggingFaceProvider::new(config)?)),
        "zai" => Ok(Box::new(ZaiProvider::new(config)?)),
        "minimax" => Ok(Box::new(MinimaxProvider::new(config)?)),
        "qwen" => Ok(Box::new(QwenProvider::new(config)?)),
        "kimi" => Ok(Box::new(KimiProvider::new(config)?)),
        "mistral" => Ok(Box::new(MistralProvider::new(config)?)),
        "ollama" => Ok(Box::new(OllamaProvider::new(config)?)),
        other => Err(Error::ConfigError(format!("unknown provider: {}", other))),
    }
}
```

## Tests

**Unit tests:**
- `provider/responses.rs`: request formatting (flat tools, input array), tool call parsing, tool result formatting, final text extraction
- Each thin provider: correct default URL and auth header

**Integration tests:**
- `provider_test.rs`: mock HTTP for responses provider, verify correct wire format
- Verify unknown provider name produces clear error

## Acceptance Criteria

- [x] `CMDIFY_PROVIDER_NAME=responses CMDIFY_MODEL_NAME=... CMDIFY_RESPONSES_URL=... cmdify find files` works
- [x] All 13 providers are functional (with appropriate API keys and models)
- [x] Unknown provider name produces a clear error message
- [x] All providers support tools where applicable
- [x] `make check` passes
