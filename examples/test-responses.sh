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

if [ -z "${CMDIFY_RESPONSES_URL:-}" ]; then
    echo "ERROR: CMDIFY_RESPONSES_URL environment variable is not set"
    echo "Example:   export CMDIFY_RESPONSES_URL=https://your-compat-endpoint.example.com"
    exit 1
fi

export CMDIFY_PROVIDER_NAME="responses"
export CMDIFY_MODEL_NAME="${CMDIFY_MODEL_NAME:-gpt-5-nano}"

echo "=== Responses Provider Integration Test ==="
echo "Provider:  $CMDIFY_PROVIDER_NAME"
echo "Model:     $CMDIFY_MODEL_NAME"
echo "Base URL:  $CMDIFY_RESPONSES_URL"
echo "Binary:    $CMDIFY_BIN"
echo ""

echo "--- Test 1: Simple command generation ---"
"$CMDIFY_BIN" "list all files in the current directory"
echo ""

echo "--- Test 2: Tool use (find_command) ---"
"$CMDIFY_BIN" "find files modified in the last 24 hours"
echo ""

echo "--- Test 3: Without API key (local model) ---"
if [ -n "${CMDIFY_RESPONSES_KEY:-}" ]; then
    echo "(skipped - CMDIFY_RESPONSES_KEY is set, unsetting for this test)"
    unset CMDIFY_RESPONSES_KEY
fi
"$CMDIFY_BIN" "show disk usage"
echo ""

echo "=== Responses Tests Complete ==="
