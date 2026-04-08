# Integration Test Scripts

These scripts exercise cmdify against live LLM provider APIs. They are intended
for manual developer validation during integration testing and are **not** run by
CI or `make check`.

## Prerequisites

- Build the binary: `cargo build --release`
- Set the required API key environment variable for the provider you want to test

## Running

```bash
# OpenRouter (requires OPENROUTER_API_KEY)
./examples/test-openrouter.sh

# HuggingFace (requires HUGGINGFACE_API_KEY)
./examples/test-huggingface.sh

# Generic completions / local Ollama (requires CMDIFY_COMPLETIONS_URL)
CMDIFY_COMPLETIONS_URL=http://localhost:11434 ./examples/test-completions.sh
```

## Customization

Override the model via `CMDIFY_MODEL_NAME`:

```bash
CMDIFY_MODEL_NAME=anthropic/claude-3.5-sonnet ./examples/test-openrouter.sh
```

Override the binary path via `CMDIFY_BIN`:

```bash
CMDIFY_BIN=./target/debug/cmdify ./examples/test-completions.sh
```

## Cost Warning

These scripts hit live APIs and may incur charges depending on your provider plan.
OpenRouter and HuggingFace bill per-token. Local providers (Ollama, LM Studio) are free.

## Adding New Providers

When implementing a new provider phase, create a corresponding script following the
same pattern: `examples/test-<provider>.sh`.
