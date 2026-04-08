# Live Testing — Examples & Smoke Tests

Live testing validates end-to-end behavior against real LLM endpoints. These tests are **never** part of `cargo test` and require no feature flags or code changes to the main binary. They live in `examples/` and are run manually.

---

## Approach

### `examples/` directory

Shell scripts that demonstrate and validate each provider. Each script:

- Sets required env vars (expects keys in the user's environment or shell profile)
- Runs a few representative prompts
- Checks that each prompt returns a non-empty response with exit code 0
- Prints pass/fail per prompt

---

## Examples

### `examples/test-completions.sh`

```sh
#!/usr/bin/env bash
set -euo pipefail

: "${CMDIFY_COMPLETIONS_URL:?CMDIFY_COMPLETIONS_URL is required}"
: "${CMDIFY_MODEL_NAME:?CMDIFY_MODEL_NAME is required}"
# CMDIFY_COMPLETIONS_KEY is optional (e.g., local Ollama needs no key)

export CMDIFY_PROVIDER_NAME=completions

BIN="${1:-./target/release/cmdify}"

prompts=(
  "list all files in the current directory"
  "find all PDF files modified in the last 7 days"
  "show disk usage for /tmp"
)

pass=0
fail=0

for prompt in "${prompts[@]}"; do
  echo -n "  $prompt ... "
  if result=$("$BIN" $prompt 2>/dev/null) && [ -n "$result" ]; then
    echo "PASS -> $result"
    ((pass++))
  else
    echo "FAIL"
    ((fail++))
  fi
done

echo ""
echo "Results: $pass passed, $fail failed"
[ "$fail" -eq 0 ]
```

### `examples/test-openai.sh`

```sh
#!/usr/bin/env bash
set -euo pipefail

: "${OPENAI_API_KEY:?OPENAI_API_KEY is required}"
: "${CMDIFY_MODEL_NAME:?CMDIFY_MODEL_NAME is required}"

export CMDIFY_PROVIDER_NAME=openai

BIN="${1:-./target/release/cmdify}"

prompts=(
  "list all files in the current directory"
  "find all PDF files modified in the last 7 days"
  "show disk usage for /tmp"
)

pass=0
fail=0

for prompt in "${prompts[@]}"; do
  echo -n "  $prompt ... "
  if result=$("$BIN" $prompt 2>/dev/null) && [ -n "$result" ]; then
    echo "PASS -> $result"
    ((pass++))
  else
    echo "FAIL"
    ((fail++))
  fi
done

echo ""
echo "Results: $pass passed, $fail failed"
[ "$fail" -eq 0 ]
```

### `examples/test-anthropic.sh`

```sh
#!/usr/bin/env bash
set -euo pipefail

: "${ANTHROPIC_API_KEY:?ANTHROPIC_API_KEY is required}"
: "${CMDIFY_MODEL_NAME:?CMDIFY_MODEL_NAME is required}"

export CMDIFY_PROVIDER_NAME=anthropic

BIN="${1:-./target/release/cmdify}"

prompts=(
  "list all files in the current directory"
  "find all PDF files modified in the last 7 days"
  "show disk usage for /tmp"
)

pass=0
fail=0

for prompt in "${prompts[@]}"; do
  echo -n "  $prompt ... "
  if result=$("$BIN" $prompt 2>/dev/null) && [ -n "$result" ]; then
    echo "PASS -> $result"
    ((pass++))
  else
    echo "FAIL"
    ((fail++))
  fi
done

echo ""
echo "Results: $pass passed, $fail failed"
[ "$fail" -eq 0 ]
```

### `examples/test-gemini.sh`

```sh
#!/usr/bin/env bash
set -euo pipefail

: "${GEMINI_API_KEY:?GEMINI_API_KEY is required}"
: "${CMDIFY_MODEL_NAME:?CMDIFY_MODEL_NAME is required}"

export CMDIFY_PROVIDER_NAME=gemini

BIN="${1:-./target/release/cmdify}"

prompts=(
  "list all files in the current directory"
  "find all PDF files modified in the last 7 days"
  "show disk usage for /tmp"
)

pass=0
fail=0

for prompt in "${prompts[@]}"; do
  echo -n "  $prompt ... "
  if result=$("$BIN" $prompt 2>/dev/null) && [ -n "$result" ]; then
    echo "PASS -> $result"
    ((pass++))
  else
    echo "FAIL"
    ((fail++))
  fi
done

echo ""
echo "Results: $pass passed, $fail failed"
[ "$fail" -eq 0 ]
```

### `examples/test-tools.sh`

Tests tool-enabled scenarios (requires Phase 2+):

```sh
#!/usr/bin/env bash
set -euo pipefail

# Uses whatever provider is configured via CMDIFY_PROVIDER_NAME and related vars
: "${CMDIFY_PROVIDER_NAME:?CMDIFY_PROVIDER_NAME is required}"
: "${CMDIFY_MODEL_NAME:?CMDIFY_MODEL_NAME is required}"

BIN="${1:-./target/release/cmdify}"

echo "=== Tool tests (find_command) ==="
echo -n "  find_command triggers on 'search with ripgrep' ... "
if result=$("$BIN" "search for TODO comments in all Rust files using ripgrep" 2>/dev/null) && [ -n "$result" ]; then
  echo "PASS -> $result"
else
  echo "FAIL"
fi

echo ""
echo "=== Tool tests (ask_user, requires -t flag or interactive terminal) ==="
echo "  Skipping: ask_user requires interactive input"
echo "  Run manually: $BIN 'archive these files'"
```

---

## Usage

```sh
# Build first
make build

# Test local model (no API key needed)
CMDIFY_MODEL_NAME=llama3 CMDIFY_COMPLETIONS_URL=http://localhost:11434 ./examples/test-completions.sh

# Test OpenAI
CMDIFY_MODEL_NAME=gpt-5-nano ./examples/test-openai.sh

# Test Anthropic
CMDIFY_MODEL_NAME=claude-sonnet-4-20250514 ./examples/test-anthropic.sh

# Test Gemini
CMDIFY_MODEL_NAME=gemini-2.5-flash ./examples/test-gemini.sh

# Test with a custom binary path
./examples/test-openai.sh ./target/debug/cmdify
```

---

## Guidelines

- Scripts require their provider's API key to be set in the environment (or shell profile). They fail fast with a clear message if missing.
- All interactive output from `ask_user` goes to stderr and is suppressed via `2>/dev/null` in automated runs. Manual runs can omit the redirect to see the full interaction.
- Scripts accept an optional binary path argument (defaults to `./target/release/cmdify`).
- Scripts exit 0 on all pass, 1 on any fail.
- Each provider script follows an identical structure — easy to copy for new providers.
- These scripts are **not** tracked as test coverage. They are validation and documentation.
