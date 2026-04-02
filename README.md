# cmdify

Turn natural language into shell commands with AI.

`cmdify` sends your plain-English description to an LLM and gets back a ready-to-run shell command. It supports multiple LLM providers, interactive clarification when the model needs more info, and command discovery to verify tools exist on your system.

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
export CMDIFY_MODEL_NAME=gpt-4o
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

Create a TOML file at `$XDG_CONFIG_HOME/cmdify/config.toml` (or `$HOME/.config/cmdify/config.toml` if `XDG_CONFIG_HOME` is not set):

```toml
provider_name = "openai"
model_name = "gpt-4o"
max_tokens = 4096
system_prompt = "/path/to/custom_prompt.txt"
```

The config file is optional. If present, its values are used as defaults that environment variables can override.

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

## CLI Flags

| Flag                  | Effect                                    |
|-----------------------|-------------------------------------------|
| `-q`, `--quiet`       | Disable the `ask_user` clarification tool |
| `-b`, `--blind`       | Disable the `find_command` discovery tool |
| `-n`, `--no-tools`    | Disable all tools                         |
| `-y`, `--yolo`        | Execute the generated command after printing it |
| `-s N`, `--spinner N` | Spinner style: 1 (default), 2 (braille), 3 (dots) |
| `-u`, `--unsafe`      | Allow potentially unsafe commands (bypasses safety check) |

## Development

```sh
make test      # Run tests
make lint      # Clippy + fmt check
make check     # lint + test
make fmt       # Auto-format code
```

## License

MIT
