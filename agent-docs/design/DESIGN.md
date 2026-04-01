# aicmd — Architecture & Design Document

## 1. Overview

`aicmd` is a native Rust CLI binary that translates natural language input into shell commands by querying LLM services. It supports multiple providers, exposes interactive tools to the model, and is configured entirely through environment variables and CLI flags.

**Related design docs:**

- [PROVIDERS.md](./PROVIDERS.md) — Provider trait, implementations, wire formats
- [TOOLS.md](./TOOLS.md) — Tool system, definitions, execution, tool call loop
- [BUILD.md](./BUILD.md) — Configuration, dependencies, build process, testing, distribution

---

## 2. High-Level Architecture

```
┌─────────────────────────────────────────────┐
│                  CLI Layer                  │
│  clap · arg parsing · flag handling         │
└──────────────────┬──────────────────────────┘
                   │
┌──────────────────▼──────────────────────────┐
│               Orchestrator                  │
│  prompt assembly · tool loop · output       │
└──┬──────────┬──────────┬────────────────────┘
   │          │          │
   │  ┌───────▼──┐  ┌────▼──────────┐
   │  │  Config  │  │   Provider    │
   │  │   Env    │  │    Trait      │
   │  └──────────┘  └───┬──┬──┬─────┘
   │                    │  │  │
   │          ┌─────────┘  │  └─────┐
   │          │      ┌─────┘        │
   │     ┌────▼──┐ ┌─▼──────┐ ┌─────▼──────┐
   │     │OpenAI │ │Anthro..│ │ Completions│
   │     └───────┘ └────────┘ │ Responses  │
   │                          │  (generic) │
   │                          └────────────┘
   │
   │  ┌────────────────────────────────────┐
   │  │          Tool System               │
   │  │  ask_user · find_command           │
   │  └────────────────────────────────────┘
```

The application follows a single-threaded async loop driven by the orchestrator.

---

## 3. Module Structure

```
src/
├── main.rs              # entry point, help message
├── cli.rs               # clap arg definitions, flag parsing
├── config.rs            # env-var config loading, provider settings
├── orchestrator.rs      # main request/response loop
├── provider/
│   ├── mod.rs           # Provider trait, factory function
│   ├── openai.rs        # OpenAI provider
│   ├── anthropic.rs     # Anthropic provider
│   ├── gemini.rs        # Google Gemini provider
│   ├── completions.rs   # generic OpenAI-compatible /completions
│   ├── responses.rs     # generic OpenAI-compatible /responses
│   ├── zai.rs           # Z.ai provider
│   ├── minimax.rs       # Minimax provider
│   ├── qwen.rs          # Qwen provider
│   ├── kimi.rs          # Kimi provider
│   ├── mistral.rs       # Mistral provider
│   ├── openrouter.rs    # OpenRouter provider
│   └── huggingface.rs   # HuggingFace provider
├── tools/
│   ├── mod.rs           # Tool trait, registry
│   ├── ask_user.rs      # interactive multiple-choice question
│   └── find_command.rs  # command discovery (command -v / which)
├── prompt.rs            # prompt assembly, exposes SYSTEM_PROMPT
└── system_prompt.txt    # system prompt text (embedded at compile time)
```

Unit tests live in each source file inside `#[cfg(test)] mod tests { ... }` blocks. Integration tests live in a top-level `tests/` directory:

```
tests/
├── config_test.rs       # integration tests for config loading
├── provider_test.rs     # integration tests with mock HTTP
├── tools_test.rs        # integration tests for tool execution
└── orchestrator_test.rs # end-to-end tests with mock provider
```

---

## 4. Core Components (Summary)

### 4.1 CLI Layer (`cli.rs`)

Uses `clap` with derive macros. Parses:

| Flag | Short | Effect |
|------|-------|--------|
| `--quiet` | `-q` | Disables the `ask_user` tool |
| `--blind` | `-b` | Disables the `find_command` tool |
| `--no-tools` | `-n` | Disables all tools |

**Flag precedence:** `-n` (`--no-tools`) takes absolute precedence over `-q` and `-b`. If `-n` is set, no tools are registered regardless of whether `-q` or `-b` are also present. The `clap` configuration should mark `-n` as conflicting with `-q` and `-b` to prevent confusing combinations.

Positional arguments (all remaining args after flags) are joined with spaces to form the user prompt.

If no positional arguments are provided, print a help/usage message and exit with code 0.

### 4.2 Configuration (`config.rs`)

See [BUILD.md — Configuration](./BUILD.md#1-configuration) for the full env var reference.

### 4.3 Provider Trait (`provider/`)

See [PROVIDERS.md](./PROVIDERS.md) for the provider trait, factory, implementations, and wire formats.

### 4.4 Tool System (`tools/`)

See [TOOLS.md](./TOOLS.md) for the tool trait, registry, definitions, execution, and tool call loop.

### 4.5 Orchestrator (`orchestrator.rs`)

Responsible for startup and driving the tool call loop. Detailed loop logic is in [TOOLS.md — Tool Call Loop](./TOOLS.md#6-tool-call-loop).

Startup sequence:

```
1. Assemble user prompt from CLI args
2. Load system prompt (compiled-in via include_str!, with optional runtime override via AICMD_SYSTEM_PROMPT)
3. Detect shell ($SHELL env var) and append to system prompt
4. Build ToolRegistry based on CLI flags (quiet, blind, no-tools)
5. Create provider instance from config
6. Enter tool call loop
```

### 4.6 System Prompt (`prompt.rs` + `system_prompt.txt`)

The system prompt is stored as a plain text file at `src/system_prompt.txt` and embedded into the binary at compile time via `include_str!()`:

```rust
pub const EMBEDDED_SYSTEM_PROMPT: &str = include_str!("system_prompt.txt");
```

This has no impact on the build process — `cargo build` picks up the file automatically and embeds its contents as a `&'static str`. No runtime file I/O is needed for the default case.

**Runtime override:** If the `AICMD_SYSTEM_PROMPT` env var is set, its value is treated as a path to a file, and the contents of that file are used instead of the compiled-in prompt. This allows testing prompt changes without recompilation and lets power-users customize behavior.

**Shell detection:** At startup, the orchestrator detects the user's shell via the `$SHELL` env var (defaulting to `"bash"` if unset) and appends it to the system prompt as context (e.g., `"The user's shell is zsh."`). This means the final system prompt is dynamically assembled at runtime, combining the base prompt text with the shell context.

The prompt instructs the model to:

- Generate a single, correct shell command from the user's natural language description.
- Use the `ask_user` tool if the request is ambiguous and needs clarification.
- Use the `find_command` tool to verify that suggested commands exist before outputting them.
- Respond with ONLY the command text (no markdown fences, no explanation) in the final answer.
- Use tool calls to gather information; only produce a final answer when confident.

---

## 5. Data Flow Sequence

```
User runs: aicmd find all pdf files larger than 10MB
                     │
                     ▼
              ┌──────────────┐
              │  Parse CLI   │
              │  quiet=false │
              │  blind=false │
              └──────┬───────┘
                     │
                     ▼
              ┌──────────────┐
              │  Load Config │
              │  from env    │
              └──────┬───────┘
                     │
                     ▼
          ┌───────────────────────┐
          │   Send to Provider    │
          │   messages array:     │
          │   System + User msg   │
          │   + tool definitions  │
          └───────────┬───────────┘
                     │
                     ▼
         ┌───────────────────────┐
         │   Provider Response   │
         │   tool_call:          │
         │     find_command(     │
         │       "fd")           │
         └───────────┬───────────┘
                     │
                     ▼
         ┌───────────────────────┐
         │   Execute Tool        │
         │   which fd → /opt/... │
         └───────────┬───────────┘
                     │
                     ▼
         ┌───────────────────────┐
         │   Send tool result    │
         │   back to provider    │
         └───────────┬───────────┘
                     │
                     ▼
         ┌───────────────────────┐
         │   Final Response      │
         │   "fd -e pdf -S +10M" │
         └───────────┬───────────┘
                     │
                     ▼
              ┌──────────────┐
              │ Print to     │
              │ stdout       │
              └──────────────┘
```

---

## 6. Open Questions

See [QUESTIONS.md](./QUESTIONS.md) for decisions that need your input.
