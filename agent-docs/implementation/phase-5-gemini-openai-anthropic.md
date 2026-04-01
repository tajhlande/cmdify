# Phase 5 — Gemini, OpenAI, Anthropic Providers

## Goal

Add the three major first-class providers, each with a distinct wire format. This phase introduces non-OpenAI-compat providers and the `AuthStyle::QueryParam` variant for Gemini.

## Scope

- **OpenAI provider**: OpenAI Completions wire format (same category as completions, but named with default URL and key)
- **Anthropic provider**: Anthropic Messages wire format (distinct format, system prompt in top-level field)
- **Gemini provider**: Google Gemini wire format (distinct format, query-param auth, `finish_reason` ambiguity resolution)
- `AuthStyle::QueryParam` variant for Gemini
- Environment variables for all three providers

## Files to Create / Modify

```
src/
├── config.rs              # MODIFY: add provider settings for OpenAI, Anthropic, Gemini
└── provider/
    ├── mod.rs             # MODIFY: factory, add new providers
    ├── openai.rs          # CREATE: thin OpenAI wrapper (completions category)
    ├── anthropic.rs       # CREATE: full Anthropic Messages implementation
    └── gemini.rs          # CREATE: full Gemini implementation
tests/
└── provider_test.rs       # MODIFY: add tests for new providers
```

## Implementation Steps

### 5.1 OpenAI provider (`src/provider/openai.rs`)

Thin wrapper around shared completions logic (same pattern as OpenRouter/HuggingFace):

- Default base URL: `https://api.openai.com`
- Auth header: `Authorization: Bearer {OPENAI_API_KEY}`
- API key env var: `OPENAI_API_KEY` (required)
- Base URL override: `OPENAI_BASE_URL`
- Delegates to `send_completions_request()`

This is straightforward since OpenAI invented the completions format.

### 5.2 Anthropic provider (`src/provider/anthropic.rs`)

Full custom implementation per `PROVIDERS.md §3.2`:

**Request format:**
- System prompt goes in top-level `"system"` field, not in messages array
- `max_tokens` is required
- Tools use `tools[].name` and `tools[].input_schema` (not `parameters`)
- Auth header: `x-api-key: {ANTHROPIC_API_KEY}` (no prefix)

**Response parsing:**
- `content[]` array with `{ type: "text", text }` or `{ type: "tool_use", id, name, input }`
- `stop_reason: "end_turn"` → `FinishReason::Stop`
- `stop_reason: "tool_use"` → `FinishReason::ToolCalls`

**Tool result format:**
- `role: "user"` with `content[]` containing `{ type: "tool_result", tool_use_id, content }`

**Private methods:**
- `format_messages()`: translate `Message` variants to Anthropic wire format (system prompt extracted from messages)
- `format_tools()`: translate `ToolDefinition` to Anthropic format (`input_schema` key)
- `parse_response()`: parse content array, extract text and tool_use parts
- `parse_finish_reason()`: map `"end_turn"` and `"tool_use"`

### 5.3 Gemini provider (`src/provider/gemini.rs`)

Full custom implementation per `PROVIDERS.md §3.3`:

**Request format:**
- `contents[]` array with `{ role, parts: [{ text }] }`
- System prompt in `system_instruction.parts[{ text }]`
- Tools in `tools[].function_declarations[]`
- Auth via query parameter: `?key={GEMINI_API_KEY}` appended to URL
- URL path: `{base_url}/v1beta/models/{model}:generateContent`

**Response parsing:**
- `candidates[].content.parts[]` with `{ text }` or `{ function_call: { name, args } }`
- **Critical**: `finish_reason: "STOP"` is used for both final answers AND tool calls. Must inspect `parts[]` for `function_call` presence to distinguish.
- Map to `FinishReason::Stop` or `FinishReason::ToolCalls` accordingly

**Tool result format:**
- `contents[].parts[]` with `{ function_response: { name, response: { content } } }`

**Private methods:**
- `format_messages()`: translate `Message` variants to Gemini format
- `format_tools()`: translate to `function_declarations` format
- `parse_response()`: parse candidates, inspect parts for text/function_call
- `build_url()`: construct URL with model name and query-param auth

### 5.4 Config updates

Add `AuthStyle::QueryParam` handling for Gemini in `ProviderSettings::from_env()`:

```rust
"gemini" => Self::query_param("GEMINI_API_KEY", "GEMINI_BASE_URL",
    "https://generativelanguage.googleapis.com", "key"),
```

### 5.5 Factory updates

```rust
"openai" => Ok(Box::new(OpenAiProvider::new(config)?)),
"anthropic" => Ok(Box::new(AnthropicProvider::new(config)?)),
"gemini" => Ok(Box::new(GeminiProvider::new(config)?)),
```

## Tests

**Unit tests:**
- `provider/anthropic.rs`: request formatting (system prompt extraction, tool schema translation, tool result formatting), response parsing (text, tool_use), finish_reason mapping
- `provider/gemini.rs`: request formatting (contents, system_instruction, function_declarations, URL construction), response parsing (text, function_call, STOP ambiguity), tool result formatting
- `provider/openai.rs`: correct defaults, delegation to shared completions

**Integration tests:**
- `provider_test.rs`: mock HTTP for each provider, verify correct wire format sent and response parsed correctly

## Acceptance Criteria

- [ ] `AICMD_PROVIDER_NAME=openai AICMD_MODEL_NAME=gpt-4 OPENAI_API_KEY=... aicmd find files` works
- [ ] `AICMD_PROVIDER_NAME=anthropic AICMD_MODEL_NAME=claude-sonnet-4-20250514 ANTHROPIC_API_KEY=... aicmd find files` works
- [ ] `AICMD_PROVIDER_NAME=gemini AICMD_MODEL_NAME=gemini-2.5-flash GEMINI_API_KEY=... aicmd find files` works
- [ ] All three providers support tools
- [ ] Gemini correctly distinguishes `STOP` for tool calls vs final answers
- [ ] Anthropic system prompt is sent in top-level field, not messages array
- [ ] Missing API keys produce clear error messages
- [ ] `make check` passes
