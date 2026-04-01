# Testing Strategy — No Live Provider Endpoints

All testing is done without connecting to real LLM endpoints. A three-layer approach provides fast, deterministic, hermetic coverage.

---

## Layer 1: Inline JSON Fixtures (Unit Tests)

**What:** Test pure serialization/deserialization functions inside each provider.

**How:** Define request and response JSON as `&str` constants. Call provider-internal functions like `format_messages()`, `format_tools()`, `parse_tool_calls()`, `parse_finish_reason()` directly. Assert the output matches expected values.

**Where:** In-source `#[cfg(test)] mod tests` blocks inside each `provider/*.rs` file.

**Covers:**
- Message formatting into provider-specific wire format
- Tool definition translation (e.g., `parameters` → `input_schema` for Anthropic)
- Tool call parsing from response JSON
- Finish reason mapping (including Gemini's `STOP` ambiguity)
- Tool result message formatting

**Example:**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tool_calls_extracts_function_calls() {
        let body: serde_json::Value = serde_json::from_str(r#"{
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_abc",
                        "type": "function",
                        "function": {
                            "name": "find_command",
                            "arguments": "{\"command\":\"fd\"}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        }"#).unwrap();

        let response = parse_completions_response(&body).unwrap();
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].name, "find_command");
        assert_eq!(response.finish_reason, FinishReason::ToolCalls);
    }
}
```

**No HTTP, no async runtime, no external dependencies.** These are the fastest tests.

---

## Layer 2: `wiremock` (HTTP Integration Tests)

**What:** Test the full HTTP round-trip through each provider — request construction, auth headers, URL path, and response parsing from a real `reqwest` call.

**How:** Spin up a `wiremock` mock server. Mount matchers for method, path, headers, and body. Return fixture JSON responses. Call `provider.send_request()` normally and assert the returned `ProviderResponse`.

**Where:** `tests/provider_test.rs` (top-level integration tests).

**Covers:**
- Correct HTTP method and URL path (e.g., `/v1/chat/completions`, `/v1/messages`, `/v1beta/models/{model}:generateContent`)
- Auth header construction (Bearer token, x-api-key, query param for Gemini)
- Request body structure (messages, tools, system prompt placement)
- Response deserialization from actual HTTP response body
- Error handling for non-200 status codes and error response bodies

**Example:**

```rust
#[tokio::test]
async fn completions_provider_sends_correct_request() {
    let mock = MockServer::start().await;
    mock.register(Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(header("authorization", "Bearer test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "choices": [{
                "message": { "role": "assistant", "content": "ls -la" },
                "finish_reason": "stop"
            }]
        })))
    ).await;

    let config = make_test_config("completions", mock.uri(), "test-key", "test-model");
    let provider = CompletionsProvider::new(&config).unwrap();
    let response = provider.send_request(&messages, &[]).await.unwrap();

    assert_eq!(response.content, Some("ls -la".into()));
    assert_eq!(response.finish_reason, FinishReason::Stop);
}
```

**Requires async runtime and `wiremock` dev-dependency.** Slower than Layer 1 but still fast (local HTTP, no network).

---

## Layer 3: `MockProvider` (Orchestrator Integration Tests)

**What:** Test the orchestrator's tool call loop, message accumulation, and output behavior without any HTTP involved.

**How:** Implement a `MockProvider` that satisfies the `Provider` trait. It returns canned `ProviderResponse` values from a configurable sequence. The orchestrator runs its full loop against this mock, executing real tools and producing real output.

**Where:** `tests/orchestrator_test.rs`.

**Covers:**
- Single-shot flow (no tool calls → print response)
- Tool call loop (tool call → tool execution → next request → final answer)
- Multiple tool calls in one response
- Multiple loop iterations (model calls find_command, then ask_user, then answers)
- Max iteration limit (10 iterations → error)
- Empty response handling
- Tool call → tool result → another tool call → final answer (multi-turn)
- `-n` / `--no-tools` flag (tool definitions not sent, single-shot)
- `-q` / `-b` flag filtering

**Example:**

```rust
struct MockProvider {
    responses: Vec<ProviderResponse>,
}

impl MockProvider {
    fn new(responses: Vec<ProviderResponse>) -> Self {
        Self { responses }
    }
}

#[async_trait]
impl Provider for MockProvider {
    async fn send_request(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<ProviderResponse> {
        // For testing, return canned responses based on call count.
        // A more sophisticated version inspects messages and returns
        // context-appropriate responses.
        Ok(self.responses[index].clone())
    }

    fn supports_tools(&self) -> bool { true }
    fn name(&self) -> &str { "mock" }
}

#[tokio::test]
async fn tool_call_loop_executes_find_command_then_returns() {
    let mock = MockProvider::new(vec![
        ProviderResponse {
            content: None,
            tool_calls: vec![ToolCall {
                id: "call_1".into(),
                name: "find_command".into(),
                arguments: serde_json::json!({ "command": "fd" }),
            }],
            finish_reason: FinishReason::ToolCalls,
        },
        ProviderResponse {
            content: Some("fd -e pdf -S +10M".into()),
            tool_calls: vec![],
            finish_reason: FinishReason::Stop,
        },
    ]);

    let registry = ToolRegistry::new(&CliFlags::default());
    let messages = vec![Message::System { content: "...".into() }, Message::User { content: "find pdfs".into() }];

    let result = run_tool_loop(&mock, &registry, messages).await.unwrap();
    assert_eq!(result, "fd -e pdf -S +10M");
}
```

**No HTTP, no JSON fixtures, no async HTTP runtime.** Tests the behavioral logic of the loop.

---

## Test Distribution by Phase

| Phase | Layer 1 | Layer 2 | Layer 3 |
|-------|---------|---------|---------|
| 1 MVP | Config, CLI, prompt, completions parsing | Completions HTTP round-trip | Basic single-shot flow |
| 2 find_command | Tool registry, find_command parsing | (covered by L1/L3) | Tool call loop with find_command |
| 3 ask_user | Registry flag filtering, ask_user parsing | (covered by L1/L3) | Tool call loop with ask_user |
| 4 OpenRouter/HF | Correct defaults per provider | HTTP header/URL per provider | (no new loop behavior) |
| 5 Gemini/OAI/Anthro | All three wire format parsers | HTTP round-trip per provider | Full loop with each provider |
| 6 Responses/Rest | Responses format parsing | Responses HTTP round-trip | Full loop with responses |
| 7 Cross-comp | (none — build verification only) | | |
| 8 CI/CD | (none — workflow verification only) | | |

---

## Principles

- **No network access in tests.** All tests pass offline, in CI, behind firewalls.
- **No API keys in tests.** Mock providers don't need credentials. `wiremock` tests use fake keys.
- **Deterministic.** No flaky tests from network latency, rate limits, or model variability.
- **Fast.** Layer 1 tests run in microseconds. Layer 2 in milliseconds. Layer 3 in milliseconds. Full suite under 5 seconds.
- **Isolated.** Each test sets up its own state. No shared mutable state between tests.
