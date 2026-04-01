# aicmd — Design Critiques

This document records critiques, inconsistencies, gaps, and design risks identified across [`AGENTS.md`](../AGENTS.md), [`DESIGN.md`](./DESIGN.md), [`PROVIDERS.md`](./PROVIDERS.md), [`TOOLS.md`](./TOOLS.md), and [`BUILD.md`](./BUILD.md).

---

## 1. Critical Inconsistency: Provider Trait Signature vs Tool Call Loop

**Severity: High — will not compile or function correctly as written**

The [`Provider`](./PROVIDERS.md) trait is defined as:

```rust
async fn send_request(
    &self,
    system_prompt: &str,
    user_prompt: &str,
    tools: &[ToolDefinition],
    tool_results: Option<Vec<ToolResult>>,
) -> Result<ProviderResponse>;
```

But the tool call loop in [`TOOLS.md — Section 6`](./TOOLS.md) shows the orchestrator building a `messages` array and calling:

```
response = provider.send_request(messages, tool_definitions)
```

These two signatures are incompatible. The trait takes four discrete arguments; the loop calls it with two. Pick one model and apply it consistently:

- **Option A (recommended):** Change the trait to accept `messages: &[Message]` + `tools: &[ToolDefinition]`, where `Message` is a provider-agnostic enum covering `System`, `User`, `Assistant`, and `ToolResult` variants. The orchestrator owns the growing message history and passes it in full each iteration.
- **Option B:** Keep the current flat signature and have each provider internally reconstruct the full message array. This hides conversation state inside the provider and makes multi-turn tool calls awkward.

---

## 2. `Mistral` Provider Is Defined but Missing from the Factory

**Severity: High — compile error if module is included**

[`AGENTS.md`](../AGENTS.md) lists Mistral as a supported provider. [`DESIGN.md — Section 3`](./DESIGN.md) includes `provider/mistral.rs` in the module structure. But [`PROVIDERS.md — Section 1`](./PROVIDERS.md) (`create_provider` factory) has no `"mistral"` arm, and the named provider table in Section 2.2 has no Mistral entry. Either add Mistral to both places or remove `mistral.rs` from the module structure.

---

## 3. `ask_user` Prompts Mixed into stdout Will Break Pipe Usage

**Severity: High — core usability flaw**

[`TOOLS.md — Section 4.1`](./TOOLS.md) states: *"All user interaction goes to stdout."* But the final shell command is also printed to stdout. A user composing commands like:

```sh
$(aicmd list files modified today)
```

will have `ask_user` prompt strings (`> [aicmd] ...`) mixed into the captured output, corrupting the command. Interactive prompt text must go to **stderr** (or directly to `/dev/tty`), keeping stdout clean for the machine-readable command output. This is standard UNIX practice for interactive CLIs that are pipe-friendly.

---

## 4. `ask_user` Display Format Is Inconsistent with Its Schema

**Severity: Medium — implementation ambiguity**

The [`ask_user`](./TOOLS.md) display example shows:

```
> [aicmd] Use fd or find?
  A) fd
  B) find
```

This implies each letter choice has an associated label ("fd", "find"). But the tool's JSON schema only defines a `choices` array of single-letter strings like `["A", "B"]`. There is no `labels` or `options` field for the descriptive text. Either:

- Add a parallel `labels` array (e.g., `["fd", "find"]`) to the schema, **or**
- Accept that choices are the full display strings (e.g., `["A: use fd", "B: use find"]`), and document that convention explicitly.

As written, the display format cannot be reproduced from the schema.

---

## 5. `ToolOutputRole` Encodes Provider-Specific Semantics in the Wrong Layer

**Severity: Medium — leaky abstraction**

[`TOOLS.md — Section 1`](./TOOLS.md) defines:

```rust
pub enum ToolOutputRole {
    Tool,
    User,
}
```

The rationale is that `ask_user` results should be returned as a `User` role message while `find_command` results use `Tool` role. However, which role is appropriate for tool results is a provider wire-format concern, not a tool concern:

- Anthropic requires tool results to be sent under `role: "user"` with `type: "tool_result"` content — regardless of the tool.
- OpenAI requires `role: "tool"` with `tool_call_id`.

Baking the role into `ToolOutput` creates a false cross-cutting distinction and will confuse implementers. The provider's `format_tool_results()` method should determine the correct role for its wire format; `ToolOutput` should not carry a role at all.

---

## 6. Tool Call Loop Has a Logical Error in Message Ordering

**Severity: Medium — incorrect conversation history**

In [`TOOLS.md — Section 6`](./TOOLS.md), the loop pseudocode reads:

```
if response.content.is_some():
    messages.append({ role: "assistant", content: response.content })
    messages.append(tool_calls)
```

This appends `content` and `tool_calls` as two separate messages, which is incorrect. In all provider formats, the assistant's partial text content and its tool calls are part of a **single** assistant message. Splitting them creates invalid conversation history. The correct behavior is: append one assistant message that contains both the content (if any) and the tool calls together.

---

## 7. `Config` Struct Eagerly Loads All Provider Credentials

**Severity: Medium — poor user experience and security hygiene**

The [`Config`](./BUILD.md) struct has a field pair for every supported provider (`openai_api_key`, `anthropic_api_key`, `gemini_api_key`, etc.), all populated at startup regardless of which provider is selected. This means:

- A misconfigured `ANTHROPIC_API_KEY` silently goes unused if you're running OpenAI, giving no feedback.
- Secrets for all providers are loaded into memory even when irrelevant.
- Adding a new provider means touching the monolithic `Config` struct.

**Recommended fix:** Load only the selected provider's configuration after `AICMD_PROVIDER_NAME` is read. Each provider should implement its own `Config::from_env()` or similar method.

---

## 8. `max_tokens` Is Required by Anthropic but Has No Configuration

**Severity: Medium — Anthropic requests will fail without it**

Anthropic's API requires `max_tokens` as a mandatory field. [`PROVIDERS.md — Section 3.2`](./PROVIDERS.md) shows a hardcoded `"max_tokens": 1024` in the wire format example, but this value is not mentioned in any configuration table, env var list ([`BUILD.md — Section 1`](./BUILD.md)), or the `Config` struct. A hardcoded 1024 tokens is too low for complex commands with long reasoning chains. This should be configurable, e.g., via `AICMD_MAX_TOKENS` with a sensible default like 4096.

---

## 9. `find_command` Uses Only `which` — Insufficient for All Environments

**Severity: Low-Medium — reliability issue on some systems**

[`TOOLS.md — Section 4.2`](./TOOLS.md) and the module comment in [`DESIGN.md`](./DESIGN.md) mention *"which / whereis"* but the implementation only uses `which`. On some minimal Linux environments (Docker containers, CI images), `which` may not be installed or may behave differently. The POSIX-standard `command -v <cmd>` (run in a shell subprocess) is more universally reliable. Consider using `command -v` as the primary lookup or as a fallback.

---

## 10. No Runtime Override for the System Prompt

**Severity: Low-Medium — limits testability and power-user flexibility**

The system prompt is compile-time-only via [`include_str!()`](./DESIGN.md). There is no mechanism to override it at runtime. This makes it difficult to:

- Test prompt changes without a full recompile.
- Power-users can't customize behavior without forking.
- Integration tests must use the compiled-in prompt.

A simple `AICMD_SYSTEM_PROMPT` env var (path to a file, or an inline string) would address this with minimal implementation cost.

---

## 11. Flag Precedence for `--no-tools` Combined with `-q`/`-b` Is Undefined

**Severity: Low — edge case, but should be documented**

[`AGENTS.md`](../AGENTS.md) and [`TOOLS.md — Section 2`](./TOOLS.md) define three flags (`-q`, `-b`, `-n`) but do not specify behavior when they are combined (e.g., `-n -q`, `-n -b`). The `ToolRegistry::new()` logic implies `-n` takes precedence (it checks `no_tools` first), but this should be explicitly documented as a behavioral contract. Additionally, `clap` could be configured to make `-n` conflict with `-q`/`-b` to prevent user confusion.

---

## 12. `AGENTS.md` Has Empty Stub Sections

**Severity: Low — documentation incompleteness**

[`AGENTS.md`](../AGENTS.md) contains two empty section headers — **"Build process"** and **"Deployments"** — with no content. These either need content (pointing to [`BUILD.md`](./BUILD.md)), or should be removed to avoid confusing agents that rely on this file for guidance.

---

## 13. No CI Configuration File Is Specified

**Severity: Low — missing operational artifact**

[`BUILD.md — Section 5.3`](./BUILD.md) says CI should run `cargo test`, `cargo clippy`, and `cargo fmt --check`, but no GitHub Actions workflow file (`.github/workflows/ci.yml` or similar) is included in the project structure. Without an actual CI definition, this is aspirational rather than enforced.

---

## 14. Open Questions in `QUESTIONS.md` Are Unresolved

**Severity: Low — design gaps that will affect implementation**

[`QUESTIONS.md`](./QUESTIONS.md) contains four open questions, all with recommendations but no recorded decisions. Specifically:

- **Question 2 (shell detection):** The recommendation is to detect `$SHELL` and inject it into the system prompt. This has implications for [`prompt.rs`](./DESIGN.md) and `system_prompt.txt` design — it means the system prompt cannot be a pure `&'static str` anymore; it must be dynamically assembled at runtime. This interaction between the recommendation and the compile-time embedding design needs to be resolved explicitly.

- **Question 4 (output format):** The recommendation is raw text only. This should be formalized in the system prompt's instructions to the model (it currently says this, per [`DESIGN.md — Section 4.6`](./DESIGN.md)), but the decision should be removed from QUESTIONS.md once confirmed.

---

## Summary Table

| # | Area | Severity | Issue |
|---|------|----------|-------|
| 1 | Provider trait vs tool loop | **High** | Incompatible call signatures |
| 2 | Mistral provider | **High** | Missing from factory and provider table |
| 3 | `ask_user` stdout | **High** | Interactive prompts corrupt piped output |
| 4 | `ask_user` schema | Medium | Display format inconsistent with JSON schema |
| 5 | `ToolOutputRole` | Medium | Provider-specific concern in wrong layer |
| 6 | Tool call loop | Medium | Content and tool calls appended as two messages |
| 7 | `Config` struct | Medium | Eagerly loads all provider credentials |
| 8 | `max_tokens` | Medium | Required by Anthropic, not configurable |
| 9 | `find_command` | Low-Med | `which` not universally reliable |
| 10 | System prompt | Low-Med | No runtime override mechanism |
| 11 | Flag precedence | Low | `-n` + `-q`/`-b` combination undefined |
| 12 | `AGENTS.md` stubs | Low | Empty "Build process" and "Deployments" sections |
| 13 | CI config | Low | CI described but not defined as a workflow file |
| 14 | Open questions | Low | Shell detection conflicts with compile-time prompt embedding |
| 15 | Tool call loop ordering | **High** | `ToolResult` messages appended before `Assistant` message |
| 16 | Gemini `finish_reason` | Medium | `"STOP"` used for both stop and tool calls — requires content inspection |
| 17 | Gemini auth model | Medium | Query-param auth doesn't fit `ProviderSettings` auth-header model |
| 18 | `Error` enum | Medium | No `IoError` variant for stdin reads and file-based prompt loading |
| 19 | Responses API statefulness | Medium | Stateful vs stateless conversation mode not specified |
| 20 | `AGENTS.md` stubs | Low | Carry-over: empty "Build process" and "Deployments" sections |

---

## Resolution Status of Original Critiques

| # | Status | Notes |
|---|--------|-------|
| 1 | ✅ Resolved | `send_request(&[Message], &[ToolDefinition])` — consistent everywhere |
| 2 | ✅ Resolved | `"mistral"` arm added to factory; Mistral in named provider table |
| 3 | ✅ Resolved | `ask_user` explicitly sends interactive text to stderr |
| 4 | ✅ Resolved | Schema now has `{ key, label }` objects; display is `{key}) {label}` |
| 5 | ✅ Resolved | `ToolOutput` is `{ content: String }` only; providers own role mapping |
| 6 | ✅ Resolved | Single `Message::Assistant`; correct ordering fixed in #15 |
| 7 | ✅ Resolved | Two-stage `Config::from_env()` loads only active provider credentials |
| 8 | ✅ Resolved | `AICMD_MAX_TOKENS` env var with 4096 default |
| 9 | ✅ Resolved | `command -v` primary, `which` as fallback |
| 10 | ✅ Resolved | `AICMD_SYSTEM_PROMPT` env var added |
| 11 | ✅ Resolved | `-n` documented as taking absolute precedence; `clap` conflict configured |
| 12 | 🔴 Not addressed | Empty stubs remain in `AGENTS.md` (user doc, outside design scope) |
| 13 | 🟡 Partially | CI workflow file path named in `BUILD.md`; actual file still absent |
| 14 | ✅ Resolved | `QUESTIONS.md` cleared; decisions reflected in design docs |
| 15 | ✅ Resolved | `Message::Assistant` now appended before `Message::ToolResult` entries |
| 16 | ✅ Resolved | Gemini `parse_finish_reason()` documented to inspect parts for `function_call` |
| 17 | ✅ Resolved | `AuthStyle` enum with `Header` and `QueryParam` variants added to `ProviderSettings` |
| 18 | ✅ Resolved | `IoError(#[from] std::io::Error)` added to `Error` enum |
| 19 | ✅ Resolved | Responses API uses stateless-only mode; documented in PROVIDERS.md §3.4 |
| 20 | 🔴 Not addressed | Carry-over of #12; empty stubs in `AGENTS.md` (user doc, outside design scope) |

---

## 15. Tool Call Loop: `ToolResult` Messages Appended Before `Assistant` Message

**Severity: High — produces invalid conversation history for all providers**

The tool call loop in [`TOOLS.md — Section 6`](./TOOLS.md#6-tool-call-loop) appends tool results to the message array **before** the assistant message that requested them:

```
for each tool_call in response.tool_calls:
    result = tool_registry.execute(...)
    messages.append(Message::ToolResult { ... })          ← appended first

messages.append(Message::Assistant { tool_calls: ... })   ← appended second
```

The "Key details" note even explicitly describes this wrong order: *"The tool results are appended to messages first, then the assistant message."*

Every provider wire format — OpenAI, Anthropic, Gemini, and Responses — requires conversation history to flow in this order:

```
[system, user, assistant (with tool_calls), tool_result_1, tool_result_2, ...]
```

Sending tool results before the assistant message that triggered them produces a conversation history that is structurally invalid. Providers will reject it or behave unpredictably.

**Fix:** Reverse the order. Append the `Message::Assistant` (with its `content` and `tool_calls`) first, then append the `Message::ToolResult` entries:

```rust
messages.push(Message::Assistant {
    content: response.content.clone(),
    tool_calls: response.tool_calls.clone(),
});
for tool_call in &response.tool_calls {
    let result = tool_registry.execute(&tool_call.name, tool_call.arguments.clone()).await?;
    messages.push(Message::ToolResult {
        tool_call_id: tool_call.id.clone(),
        name: tool_call.name.clone(),
        content: result.content,
    });
}
```

---

## 16. Gemini `finish_reason: "STOP"` Is Ambiguous — Cannot Distinguish Stop from Tool Calls

**Severity: Medium — tool call detection logic will fail for Gemini**

The OpenAI Completions API uses distinct `finish_reason` values: `"stop"` for a final answer and `"tool_calls"` when the model wants to call tools. The [`FinishReason`](./PROVIDERS.md) enum maps cleanly to these.

Google Gemini, however, returns `finish_reason: "STOP"` in **both** cases — when the model is done, and when it has returned function calls in the `parts` array. The [`PROVIDERS.md — Section 3.3`](./PROVIDERS.md) wire format example shows `"finish_reason": "STOP"` even for a tool-call response.

This means the Gemini provider's `parse_finish_reason()` cannot determine `FinishReason::ToolCalls` by inspecting `finish_reason` alone. It must also inspect `candidates[0].content.parts` for the presence of `function_call` objects. This cross-field detection logic is more complex than the trait's method signature implies (a single `parse_finish_reason(response)` method) and is not documented anywhere in the design.

**Fix:** Document that the Gemini provider's response parsing requires a unified `parse_response()` step that inspects both the content parts and the finish reason field together, before populating `ProviderResponse`. Alternatively, `parse_finish_reason` could accept the full partially-parsed response (content + finish_reason string) so the Gemini implementation can apply the dual-inspection logic.

---

## 17. Gemini API Key Auth Is a Query Parameter — Incompatible with `ProviderSettings` Auth-Header Model

**Severity: Medium — Gemini provider cannot use the shared settings model**

[`BUILD.md — Section 1.3`](./BUILD.md) defines `ProviderSettings` with an `api_key: String` and `base_url: String` field pair. [`PROVIDERS.md — Section 2.2`](./PROVIDERS.md) describes named providers as "thin wrappers that set the correct default base URL and auth header."

Gemini's auth is fundamentally different: it is a **query parameter** (`?key=<GEMINI_API_KEY>`), not a request header. There is no `Authorization: Bearer ...` or `x-api-key: ...` header involved. The shared `ProviderSettings` model and the "thin wrapper" pattern assume all providers send auth as a header, which is incorrect for Gemini.

The Gemini provider must append the API key as a query parameter when constructing the request URL. This is not represented in `ProviderSettings`, and the implicit contract that providers differ only in `base_url` and `auth_header` breaks here.

**Fix:** Either (a) give `ProviderSettings` an `auth_style: AuthStyle` field with variants like `BearerHeader`, `XApiKeyHeader`, and `QueryParam` — allowing the provider to select the correct auth mechanism at build time — or (b) acknowledge in the design that Gemini is a fully custom implementation that does not share the thin-wrapper pattern used by the OpenAI Completions category.

---

## 18. `Error` Enum Has No `IoError` Variant

**Severity: Medium — I/O failures lose error context or require misuse of other variants**

The [`Error`](./BUILD.md#3-error-handling) enum covers `ConfigError`, `ProviderError`, `ToolError`, `ResponseError`, and `HttpError(#[from] reqwest::Error)`, but has no `std::io::Error` coverage.

Two distinct code paths produce I/O errors:

1. **`ask_user` stdin reads** — reading user input in [`tools/ask_user.rs`](./DESIGN.md) can fail with an `io::Error` (e.g., stdin closed, TTY unavailable, timeout handling with `spawn_blocking`).
2. **`AICMD_SYSTEM_PROMPT` file loading** — the system prompt file path feature in [`prompt.rs`](./DESIGN.md) reads a file at startup, which can fail with `io::Error` (file not found, permission denied, etc.).

Without an `IoError` variant, these errors must be mapped into `ToolError(String)` or `ConfigError(String)` with `to_string()`, discarding the original error type and making it harder to pattern-match on specific failure modes in tests.

**Fix:** Add `#[error("io error: {0}")] IoError(#[from] std::io::Error)` to the `Error` enum.

---

## 19. OpenAI Responses API Conversation Statefulness Is Unspecified

**Severity: Medium — implementation choice with significant behavioral and complexity implications**

The OpenAI `/v1/responses` API supports two conversation modes:

- **Stateless:** Send the full conversation history in `input` on every request (same pattern as `/v1/chat/completions`).
- **Stateful:** Send only the new input items, referencing the prior turn via `previous_response_id`. The API server maintains history server-side.

[`PROVIDERS.md — Section 3.4`](./PROVIDERS.md) shows a request with a full `input` array, implying stateless mode. However, the stateful mode is the idiomatic pattern for this API and avoids re-sending potentially large message histories on every tool-call iteration.

If stateless mode is chosen, the `responses` provider must accumulate and re-send the full growing `input` array each iteration, including prior `function_call` and `function_call_output` items — which is not the same format as appending `Message` variants shown in the loop. If stateful mode is chosen, the responses provider needs to persist `response.id` across loop iterations and structure requests differently from all other providers.

Neither choice is documented, and the implications for the `responses` provider's implementation are non-trivial either way.

**Fix:** Explicitly choose stateless mode (full re-send of `input` each turn) for simplicity and consistency with the other providers' patterns, and document this choice in [`PROVIDERS.md`](./PROVIDERS.md). Note that this means the `responses` provider must translate the full `&[Message]` slice into its `input` wire format on each call, including the special `function_call_output` item type for `Message::ToolResult`.
