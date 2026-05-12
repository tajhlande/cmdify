# cmdify — Architecture & Design Document

## 1. Overview

`cmdify` is a native Rust CLI binary that translates natural language input into shell commands by querying LLM services. It supports multiple providers, exposes interactive tools to the model, and is configured entirely through environment variables and CLI flags.

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
│   ├── huggingface.rs   # HuggingFace provider
│   └── ollama.rs        # Ollama provider (local, no auth)
├── tools/
│   ├── mod.rs           # Tool trait, registry
│   ├── ask_user.rs      # interactive multiple-choice question
│   └── find_command.rs  # command discovery (command -v / which)
├── safety.rs             # unsafe command pattern detection
├── setup.rs              # interactive config wizard
├── history.rs            # prompt history (XDG cache, rustyline integration)
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
| `--yolo` | `-y` | Execute the generated command after printing it |
| `--spinner N` | `-s N` | Spinner style: 1 (default), 2 (braille), 3 (dots) |
| `--unsafe` | `-u` | Allow potentially unsafe commands (bypasses safety check) |
| `--setup` | — | Run interactive setup wizard (requires interactive terminal) |
| `--interactive` | `-i` | Read command description from interactive prompt instead of positional args |

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
2. Load system prompt (compiled-in via include_str!, with optional runtime override via CMDIFY_SYSTEM_PROMPT)
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

**Runtime override:** If the `CMDIFY_SYSTEM_PROMPT` env var is set, its value is treated as a path to a file, and the contents of that file are used instead of the compiled-in prompt. This allows testing prompt changes without recompilation and lets power-users customize behavior.

**Shell detection:** At startup, the orchestrator detects the user's shell via the `$SHELL` env var (defaulting to `"bash"` if unset) and appends it to the system prompt as context (e.g., `"The user's shell is zsh."`). This means the final system prompt is dynamically assembled at runtime, combining the base prompt text with the shell context.

The prompt instructs the model to:

- Generate a single, correct shell command from the user's natural language description.
- Use the `ask_user` tool if the request is ambiguous and needs clarification.
- Use the `find_command` tool to verify that suggested commands exist before outputting them.
- Respond with ONLY the command text (no markdown fences, no explanation) in the final answer.
- Use tool calls to gather information; only produce a final answer when confident.

### 4.7 Safety Check (`safety.rs`)

Inspects generated commands for potentially dangerous patterns before outputting or executing them. See [Phase 5 — Safety Check](../implementation/phase-5-safety-check.md) for full design.

**Behavior:**
- By default, if the generated command matches an unsafe pattern, cmdify prints an error to stderr and exits with code 1. The error includes the matched pattern and instructions to rerun with `--unsafe` (`-u`).
- When `-u` / `--unsafe` is passed, the safety check is skipped entirely.
- The safety check runs in `main.rs` after the orchestrator returns and before printing/executing the command.

**Configuration:** Supports the `CMDIFY_UNSAFE` env var and `unsafe` config file field. Precedence: CLI flag > env var > config file > default (false).

**Pattern categories:** Recursive delete (`rm -rf /`), disk destruction (`dd`, `mkfs`), system shutdown/reboot, privilege escalation writes, force kill all processes, package removal. The pattern list is conservative (prefers false positives over false negatives) and stored as a static array in the safety module.

### 4.8 Setup Wizard (`setup.rs`)

Interactive config wizard that creates or updates `~/.config/cmdify/config.toml`. See [Phase 11 — Interactive Setup](../implementation/phase-11-interactive-setup.md) for full design.

**Behavior:**
- `--setup` flag enters setup mode on an interactive terminal. On a non-interactive terminal, exits with an error.
- When no config file exists and the terminal is interactive (and `--quiet` is not set), cmdify auto-enters setup mode before processing the prompt.
- When no config file exists and the terminal is NOT interactive, cmdify prints a hint message to stderr suggesting `cmdify --setup`, unless `--quiet` is set.
- Setup prompts for provider, model name, max tokens, and optional system prompt file. Existing config values are used as defaults.
- API keys are NEVER written to the config file. Setup prints the appropriate `export` command for the user's shell profile.
- All setup prompts go to stderr; stdout remains clean.

### 4.9 Interactive Input

When `-i` / `--interactive` is passed, cmdify reads the command description from an interactive prompt instead of positional arguments:

1. Prints "Enter command description" to stderr, followed by a `> ` prompt.
2. Uses `rustyline` for full line editing (arrow keys, home/end, undo, Ctrl-R history search).
3. If the user presses Ctrl-C or Ctrl-D, or enters an empty line, cmdify exits without running a command.
4. If cmdify is not running in an interactive terminal (stdin is not a TTY), it exits with an error.
5. `-i` / `--interactive` conflicts with positional arguments — clap rejects the combination at parse time.

This mode is useful when the command description contains characters that would otherwise be interpreted by the shell (quotes, backticks, pipes, etc.), since the input is read as a raw string without shell parsing.

### 4.10 Command History (`history.rs`)

All user prompts are saved to a persistent history file, regardless of input mode (CLI arguments or interactive).

**Location:** `$XDG_CACHE_HOME/cmdify/history.txt`, falling back to `$HOME/.cache/cmdify/history.txt`.

**Behavior:**
- **CLI mode:** After assembling the user prompt from positional arguments, the prompt is appended to the history file.
- **Interactive mode:** `rustyline` manages the history file directly, providing up/down arrow navigation and Ctrl-R reverse search across previous inputs.
- Both modes write to the same file, so interactive history includes prompts entered via CLI arguments.
- The history file is non-essential (cache); it can be deleted without affecting cmdify's operation.

---

## 5. Data Flow Sequence

```
User runs: cmdify find all pdf files larger than 10MB
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
