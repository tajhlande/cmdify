# cmdify — Provider Design

## 1. Provider Trait (`provider/mod.rs`)

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

The orchestrator owns the full conversation history and passes it to the provider on each request. Each provider implementation translates `Message` variants into its wire format.

### Shared Types

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

`Message::ToolResult` is a single variant for all tool results. Each provider's `format_tool_results()` maps this to the correct role for its wire format (e.g., `role: "tool"` for OpenAI, `role: "user"` with `type: "tool_result"` for Anthropic). The tool system and orchestrator remain provider-agnostic.

### Factory Function

```rust
pub fn create_provider(config: &Config) -> Result<Box<dyn Provider>> {
    match config.provider_name.as_str() {
        "openai" => Ok(Box::new(OpenAiProvider::new(config)?)),
        "anthropic" => Ok(Box::new(AnthropicProvider::new(config)?)),
        "gemini" => Ok(Box::new(GeminiProvider::new(config)?)),
        "completions" => Ok(Box::new(CompletionsProvider::new(config)?)),
        "responses" => Ok(Box::new(ResponsesProvider::new(config)?)),
        "zai" => Ok(Box::new(ZaiProvider::new(config)?)),
        "minimax" => Ok(Box::new(MinimaxProvider::new(config)?)),
        "qwen" => Ok(Box::new(QwenProvider::new(config)?)),
        "kimi" => Ok(Box::new(KimiProvider::new(config)?)),
        "mistral" => Ok(Box::new(MistralProvider::new(config)?)),
        "openrouter" => Ok(Box::new(OpenRouterProvider::new(config)?)),
        "huggingface" => Ok(Box::new(HuggingFaceProvider::new(config)?)),
        "ollama" => Ok(Box::new(OllamaProvider::new(config)?)),
        other => Err(Error::ConfigError(format!("unknown provider: {}", other))),
    }
}
```

---

## 2. Provider Implementations

### 2.1 Category Groups

Providers fall into three wire-format categories:

| Category | Wire Format | Providers |
|----------|-------------|-----------|
| **OpenAI Completions** | `POST /v1/chat/completions` | `openai`, `completions`, `zai`, `minimax`, `qwen`, `kimi`, `mistral`, `openrouter`, `huggingface`, `ollama` |
| **Anthropic Messages** | `POST /v1/messages` | `anthropic` |
| **OpenAI Responses** | `POST /v1/responses` | `responses` |
| **Gemini** | `POST /v1beta/models/{model}:generateContent` | `gemini` |

### 2.2 Named Providers

Named providers are thin wrappers that set the correct default base URL and auth header, then delegate to one of the category implementations:

| Provider | Base URL | Category | Auth Header |
|----------|----------|----------|-------------|
| `openai` | `https://api.openai.com` | OpenAI Completions | `Authorization: Bearer <OPENAI_API_KEY>` |
| `anthropic` | `https://api.anthropic.com` | Anthropic Messages | `x-api-key: <ANTHROPIC_API_KEY>` |
| `gemini` | `https://generativelanguage.googleapis.com` | Gemini | `?key=<GEMINI_API_KEY>` (query param) |
| `openrouter` | `https://openrouter.ai/api` | OpenAI Completions | `Authorization: Bearer <OPENROUTER_API_KEY>` |
| `huggingface` | `https://router.huggingface.co/v1` | OpenAI Completions | `Authorization: Bearer <HUGGINGFACE_API_KEY>` |
| `zai` | `https://api.z.ai` | OpenAI Completions | `Authorization: Bearer <ZAI_API_KEY>` |
| `minimax` | `https://api.minimax.chat` | OpenAI Completions | `Authorization: Bearer <MINIMAX_API_KEY>` |
| `qwen` | `https://dashscope.aliyuncs.com/compatible-mode` | OpenAI Completions | `Authorization: Bearer <QWEN_API_KEY>` |
| `kimi` | `https://api.moonshot.cn` | OpenAI Completions | `Authorization: Bearer <KIMI_API_KEY>` |
| `mistral` | `https://api.mistral.ai` | OpenAI Completions | `Authorization: Bearer <MISTRAL_API_KEY>` |
| `ollama` | `http://localhost:11434` | OpenAI Completions | *(none)* |

Each named provider allows overriding the base URL via `_<PROVIDER>_BASE_URL`.

### 2.3 Generic Providers

The generic providers have no default URL — they require the user to set the URL explicitly:

| Provider | URL Env Var | Key Env Var |
|----------|-------------|-------------|
| `completions` | `CMDIFY_COMPLETIONS_URL` | `CMDIFY_COMPLETIONS_KEY` |
| `responses` | `CMDIFY_RESPONSES_URL` | `CMDIFY_RESPONSES_KEY` |

---

## 3. Wire Formats

### 3.1 OpenAI Completions Format

**Request:**

```json
{
  "model": "gpt-4",
  "messages": [
    { "role": "system", "content": "..." },
    { "role": "user", "content": "find all pdf files" }
  ],
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "ask_user",
        "description": "Ask the user a clarifying question.",
        "parameters": { "type": "object", "..." : "..." }
      }
    }
  ]
}
```

**Response with tool calls:**

```json
{
  "choices": [
    {
      "message": {
        "role": "assistant",
        "content": null,
        "tool_calls": [
          {
            "id": "call_abc123",
            "type": "function",
            "function": {
              "name": "find_command",
              "arguments": "{\"command\":\"fd\"}"
            }
          }
        ]
      },
      "finish_reason": "tool_calls"
    }
  ]
}
```

**Tool result message (sent back):**

```json
{
  "role": "tool",
  "tool_call_id": "call_abc123",
  "content": "/opt/homebrew/bin/fd"
}
```

**Final response (no tool calls):**

```json
{
  "choices": [
    {
      "message": {
        "role": "assistant",
        "content": "fd -e pdf -S +10M"
      },
      "finish_reason": "stop"
    }
  ]
}
```

### 3.2 Anthropic Messages Format

**Request:**

```json
{
  "model": "claude-sonnet-4-20250514",
  "max_tokens": 4096,
  "system": "You are a shell command generator.",
  "messages": [
    { "role": "user", "content": "find all pdf files" }
  ],
  "tools": [
    {
      "name": "ask_user",
      "description": "Ask the user a clarifying question.",
      "input_schema": { "type": "object", "..." : "..." }
    }
  ]
}
```

Note: system prompt goes in the top-level `system` field, not in the messages array.

**Response with tool calls:**

```json
{
  "content": [
    {
      "type": "tool_use",
      "id": "toolu_abc123",
      "name": "find_command",
      "input": { "command": "fd" }
    }
  ],
  "stop_reason": "tool_use"
}
```

**Tool result message (sent back):**

```json
{
  "role": "user",
  "content": [
    {
      "type": "tool_result",
      "tool_use_id": "toolu_abc123",
      "content": "/opt/homebrew/bin/fd"
    }
  ]
}
```

**Final response (no tool calls):**

```json
{
  "content": [
    { "type": "text", "text": "fd -e pdf -S +10M" }
  ],
  "stop_reason": "end_turn"
}
```

### 3.3 Google Gemini Format

**Request:**

```json
{
  "contents": [
    {
      "role": "user",
      "parts": [{ "text": "find all pdf files" }]
    }
  ],
  "system_instruction": {
    "parts": [{ "text": "You are a shell command generator." }]
  },
  "tools": [
    {
      "function_declarations": [
        {
          "name": "ask_user",
          "description": "Ask the user a clarifying question.",
          "parameters": { "type": "object", "..." : "..." }
        }
      ]
    }
  ]
}
```

**Response with tool calls:**

```json
{
  "candidates": [
    {
      "content": {
        "role": "model",
        "parts": [
          {
            "function_call": {
              "name": "find_command",
              "args": { "command": "fd" }
            }
          }
        ]
      },
  "finish_reason": "STOP"
}
```

**Important:** Gemini returns `finish_reason: "STOP"` for both tool-call responses and final text responses. The Gemini provider's `parse_finish_reason()` must inspect the response parts to distinguish the two: if any part contains a `function_call` key, return `FinishReason::ToolCalls`; otherwise return `FinishReason::Stop`.

### 3.4 OpenAI Responses Format

**Conversation mode:** Stateless only. The orchestrator owns the full `Message` array and sends it as the complete `input` on every request. No `previous_response_id` is ever sent — there is no cross-invocation persistence. This is consistent with how all other providers operate.

Stateless mode is the default for the Responses API (stateful is opt-in via `previous_response_id`). Any provider exposing a `/responses` endpoint that follows the OpenAI spec will accept a full `input` array. If a provider rejects stateless input, the Responses provider returns a `ProviderError` and the binary exits with code 1.

**Request:**

```json
{
  "model": "gpt-4",
  "input": [
    { "role": "system", "content": "You are a shell command generator." },
    { "role": "user", "content": "find all pdf files" }
  ],
  "tools": [
    {
      "type": "function",
      "name": "ask_user",
      "description": "Ask the user a clarifying question.",
      "parameters": { "type": "object", "..." : "..." }
    }
  ]
}
```

Note: the responses API uses a flat `tools` array where each tool has its properties at the top level, not nested under `function`.

**Response with tool calls:**

```json
{
  "output": [
    {
      "type": "function_call",
      "id": "fc_abc123",
      "call_id": "call_abc123",
      "name": "find_command",
      "arguments": "{\"command\":\"fd\"}"
    }
  ]
}
```

**Tool result message (sent back as a new input item):**

```json
{
  "type": "function_call_output",
  "call_id": "call_abc123",
  "output": "/opt/homebrew/bin/fd"
}
```

**Final response (no tool calls):**

```json
{
  "output": [
    {
      "type": "message",
      "content": [{ "type": "output_text", "text": "fd -e pdf -S +10M" }]
    }
  ]
}
```

---

## 4. Tool Translation Layer

Each provider implements private methods to translate the provider-agnostic `Message`, `ToolDefinition`, and `ToolCall` types into their wire format:

```rust
fn format_tools(&self, definitions: &[ToolDefinition]) -> serde_json::Value;
fn format_messages(&self, messages: &[Message]) -> serde_json::Value;
fn parse_tool_calls(&self, response: &serde_json::Value) -> Vec<ToolCall>;
fn parse_finish_reason(&self, response: &serde_json::Value) -> FinishReason;
```

This keeps provider-specific serialization encapsulated while the orchestrator and tool system remain provider-agnostic.

To reduce duplication, the **OpenAI Completions** category shares a common implementation. Named providers in this category are thin structs that set `base_url` and `auth_header` fields, then call the shared completions logic. Only `anthropic`, `gemini`, and `responses` require fully distinct serialization code.
