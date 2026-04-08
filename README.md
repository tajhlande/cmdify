# cmdify

Turn natural language into shell commands with AI.

`cmdify` sends your plain-English description to an LLM and gets back a ready-to-run shell command. It supports multiple LLM providers, interactive clarification when the model needs more info, and command discovery to verify tools exist on your system.

I built this tool for two reasons:
1. I wanted a tool like this, with the behavior it has
2. I wanted to see if I could guide coding a tool in a language I do not know. I have not learned or used Rust before doing this project

This project is being built with OpenCode and GLM 5 Turbo. 

## Roadmap

| Phase | Status | Title                      | Scope                                                      | Key Deliverables             |
|-------|--------|----------------------------|------------------------------------------------------------|------------------------------|
| 1     | ✅      | Minimal MVP                | `/completions` provider, no tools                          | Working binary, basic UX, env config |
| 2     | ✅      | `find_command` Tool        | Add command discovery tool                                 | Tool trait, registry, tool call loop |
| 3     | ✅      | `ask_user` Tool            | Add interactive clarification tool                         | Interactive stdin/stderr UX |
| 4     | ✅      | Tool Levels                | Numbered tool level system, `--list-tools`                 | Progressive tool disclosure, config |
| 5     | ✅      | Safety Check               | Modular prompt, LLM guidance, semantic checks              | Three-layer safety, system prompt split, shlex tokenization |
| 6     | ✅      | OpenRouter & HuggingFace   | Two more OpenAI-compat providers                           | Named provider pattern, shared completions impl |
| 7     | ✅      |  Gemini, OpenAI, Anthropic | First-class providers, distinct wire formats               | Three new providers, AuthStyle::QueryParam |
| 8     | ✅      | Responses & Remaining      | Responses API + Z.ai, Minimax, Qwen, Kimi, Mistral, Ollama | Full provider coverage |
| 9     | ⬜      | Cross-Compilation          | Build targets for all platforms                            | Makefile dist, Raspbian arm, Apple Intel/Silicon |
| 10    | ⬜      | CI/CD & Distribution       | GitHub Actions, releases, polish                           | Automated testing, release workflow, docs |
| 11    | ⬜      | Interactive Setup          | `--setup` flag, first-run detection, config wizard         | Setup module, interactive prompts, config file creation |
| 12    | ✅      | Debug Mode                 | Debug logging, `--debug` flag                              | Debug logging module, stderr trace output, configurable verbosity |

## Features

- **Multiple providers** — OpenAI, Anthropic, Google Gemini, Mistral, Qwen, Kimi, OpenRouter, HuggingFace, Z.ai, Minimax, Ollama, and any OpenAI-compatible `/completions` or `/responses` endpoint.
- **Interactive tools** — The model can ask clarifying questions (multiple-choice) and look up commands on your system (`which`/`command -v`).
- **Zero runtime deps** — Static binary via Rust + rustls. No OpenSSL, no shared libraries.
- **Pipe-friendly** — All interactive prompts go to stderr; only the final command goes to stdout, so `$(cmdify "find all pdf files")` works.

## Building

Requires [Rust](https://rustup.rs/).

```sh
# Release build
make build

# Debug build
make dev

# Install to ~/.cargo/bin
make install
```

Or directly with cargo:

```sh
cargo install --path .
```

## Quick Start

Set your provider and model, then run:

```sh
export CMDIFY_PROVIDER_NAME=openai
export CMDIFY_MODEL_NAME=gpt-5-nano
export OPENAI_API_KEY=sk-...

cmdify "find all pdf files larger than 10MB"
# Output: find . -name "*.pdf" -size +10M
```

Use the output in a subshell:

```sh
eval "$(cmdify "list all docker containers sorted by size")"
```

## Configuration

Configuration uses a layered approach: **environment variables > config file > defaults**. API keys must be set via environment variables; the config file stores non-secret settings only.

### Config File

Create a TOML file at `$XDG_CONFIG_HOME/cmdify/config.toml` (or `$HOME/.config/cmdify/config.toml` if `XDG_CONFIG_HOME` is not set). Below is a minimal config file:

```toml
provider_name = "openai"
model_name = "gpt-5-nano"
```

The config file is optional. If present, its values are used as defaults that environment variables 
or command line options can override. There is a full example config file at [config.example.toml](config.example.toml)

### Environment Variables

**Required** (can be set in config file instead):

| Variable               | Description                                                                             |
|------------------------|-----------------------------------------------------------------------------------------|
| `CMDIFY_PROVIDER_NAME` | Provider identifier (`openai`, `anthropic`, `gemini`, `completions`, `responses`, etc.) |
| `CMDIFY_MODEL_NAME`    | Model to use (e.g. `gpt-4o`, `claude-sonnet-4-20250514`, `gemini-2.0-flash`)            |

**API Keys** (env vars only):

| Provider            | Key Variable             | Base URL Variable         |
|---------------------|--------------------------|---------------------------|
| OpenAI              | `OPENAI_API_KEY`         | `OPENAI_BASE_URL`         |
| Anthropic           | `ANTHROPIC_API_KEY`      | `ANTHROPIC_BASE_URL`      |
| Gemini              | `GEMINI_API_KEY`         | `GEMINI_BASE_URL`         |
| Mistral             | `MISTRAL_API_KEY`        | `MISTRAL_BASE_URL`        |
| Qwen                | `QWEN_API_KEY`           | `QWEN_BASE_URL`           |
| Kimi                | `KIMI_API_KEY`           | `KIMI_BASE_URL`           |
| OpenRouter          | `OPENROUTER_API_KEY`     | `OPENROUTER_BASE_URL`     |
| HuggingFace         | `HUGGINGFACE_API_KEY`    | `HUGGINGFACE_BASE_URL`    |
| Z.ai                | `ZAI_API_KEY`            | `ZAI_BASE_URL`            |
| Minimax             | `MINIMAX_API_KEY`        | `MINIMAX_BASE_URL`        |
| Generic completions | `CMDIFY_COMPLETIONS_KEY` | `CMDIFY_COMPLETIONS_URL`  |
| Generic responses   | `CMDIFY_RESPONSES_KEY`   | `CMDIFY_RESPONSES_URL`    |
| Ollama              | *(none)*                 | `OLLAMA_BASE_URL`         |

Base URL variables are optional — each provider has a sensible default.

**Optional** (env var or config file):

| Variable               | Default       | Description                                          |
|------------------------|---------------|------------------------------------------------------|
| `CMDIFY_MAX_TOKENS`    | 16384        | Max tokens for providers that require it             |
| `CMDIFY_SYSTEM_PROMPT` | (compiled-in) | Override the default system prompt with the contents |
| `CMDIFY_SPINNER`       | 1             | Spinner style: 1, 2 (braille), or 3 (dots) |

There is an example environment file at [env.example](env.example).
Ensure that you have appropriate access restrictions on this file if you have any secrets stored in it,
like API keys.

## CLI Flags

| Flag                    | Effect                                                     |
|-------------------------|------------------------------------------------------------|
| `-c`, `--config FILE`   | Path to config file (must exist)                           |
| `-t N`, `--tools N`     | Tool level: 0 (none), 1 (core, default), 2 (local), 3 (system) |
| `--list-tools`          | List all available tools by level and exit                 |
| `-q`, `--quiet`         | Disable the `ask_user` clarification tool                  |
| `-b`, `--blind`         | Disable the `find_command` discovery tool                  |
| `-n`, `--no-tools`      | Disable all tools                                          |
| `-u`, `--unsafe`        | Allow potentially unsafe commands (bypasses safety check)   |
| `-y`, `--yolo`          | Execute the generated command after printing it            |
| `-d`, `--debug`         | Enable debug logging to stderr (`-d` basic, `-dd` verbose) |
| `-s N`, `--spinner N`   | Spinner style: 1 (default bar), 2 (braille), 3 (dots)      |

## Development

```sh
make test      # Run tests
make lint      # Clippy + fmt check
make check     # lint + test
make fmt       # Auto-format code
```

## License

Apache 2.0
