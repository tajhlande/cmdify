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

if [ -z "${GEMINI_API_KEY:-}" ]; then
    echo "ERROR: GEMINI_API_KEY environment variable is not set"
    echo "Get an API key from https://aistudio.google.com/apikey"
    exit 1
fi

export CMDIFY_PROVIDER_NAME="gemini"
export CMDIFY_MODEL_NAME="${CMDIFY_MODEL_NAME:-gemini-2.5-flash}"

echo "=== Gemini Integration Test ==="
echo "Provider:  $CMDIFY_PROVIDER_NAME"
echo "Model:     $CMDIFY_MODEL_NAME"
echo "Binary:    $CMDIFY_BIN"
echo ""

echo "--- Test 1: Simple command generation ---"
"$CMDIFY_BIN" "list all files in the current directory"
echo ""

echo "--- Test 2: Tool use (find_command) ---"
"$CMDIFY_BIN" "find files modified in the last 24 hours"
echo ""

echo "--- Test 3: Custom base URL (if GEMINI_BASE_URL is set) ---"
if [ -n "${GEMINI_BASE_URL:-}" ]; then
    echo "Custom base URL: $GEMINI_BASE_URL"
    "$CMDIFY_BIN" "show the current git branch"
else
    echo "(skipped - GEMINI_BASE_URL not set)"
fi
echo ""

echo "=== Gemini Tests Complete ==="
