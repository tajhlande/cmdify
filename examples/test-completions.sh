#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CMDIFY_BIN="${CMDIFY_BIN:-$PROJECT_ROOT/target/release/cmdify}"

if [ ! -f "$CMDIFY_BIN" ]; then
    echo "ERROR: cmdify binary not found at $CMDIFY_BIN"
    echo "Build with: cargo build --release"
    exit 1
fi

if [ -z "${CMDIFY_COMPLETIONS_URL:-}" ]; then
    echo "ERROR: CMDIFY_COMPLETIONS_URL environment variable is not set"
    echo "For Ollama:  export CMDIFY_COMPLETIONS_URL=http://localhost:11434"
    echo "For LM Studio: export CMDIFY_COMPLETIONS_URL=http://localhost:1234"
    exit 1
fi

export CMDIFY_PROVIDER_NAME="completions"
export CMDIFY_MODEL_NAME="${CMDIFY_MODEL_NAME:-llama3}"

echo "=== Completions Provider Integration Test ==="
echo "Provider:  $CMDIFY_PROVIDER_NAME"
echo "Model:     $CMDIFY_MODEL_NAME"
echo "Base URL:  $CMDIFY_COMPLETIONS_URL"
echo "Binary:    $CMDIFY_BIN"
echo ""

echo "--- Test 1: Simple command generation ---"
"$CMDIFY_BIN" "list all files in the current directory"
echo ""

echo "--- Test 2: Tool use (find_command) ---"
"$CMDIFY_BIN" "find files modified in the last 24 hours"
echo ""

echo "--- Test 3: Without API key (local model) ---"
if [ -n "${CMDIFY_COMPLETIONS_KEY:-}" ]; then
    echo "(skipped - CMDIFY_COMPLETIONS_KEY is set, unsetting for this test)"
    unset CMDIFY_COMPLETIONS_KEY
fi
"$CMDIFY_BIN" "show disk usage"
echo ""

echo "=== Completions Tests Complete ==="
