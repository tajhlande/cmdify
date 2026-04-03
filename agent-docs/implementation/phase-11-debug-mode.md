# Phase 11 — Debug Mode

## Goal

Add a debug/trace logging mode that emits diagnostic messages to stderr, giving users and developers visibility into cmdify's internal behavior — config resolution, API requests/responses, tool calls, and error details.

## Scope

- `--debug` / `-d` CLI flag to enable debug output
- `CMDIFY_DEBUG` env var (for config file and scripting use)
- `debug = true` option in `config.toml`
- All three follow existing precedence: CLI > env var > config file > default (`false`)
- Debug output goes to stderr only, never stdout (stdout is reserved for the generated command)
- Zero-cost when disabled (no allocations or I/O unless debug is active)

## Design Decisions

- **No external logging crate** — use a lightweight internal macro or a minimal `stderr`-based logger to avoid adding dependencies. The `log` + `env_logger` crates are common Rust choices but add crate weight; a simple `eprintln!`-based macro behind a `static AtomicBool` gate is sufficient.
- **Structured prefixes** — each log line includes a `[cmdify::module]` prefix and millisecond timestamp so output is grep-able.
- **Levels** — single on/off toggle for now (no trace/debug/info/warn hierarchy). If needed later, levels can be added without breaking the interface.
- **What gets logged** (when enabled):
  - Config resolution: which file was loaded (or none), effective values for provider/model/max_tokens
  - Provider: request URL (with API key masked as `***`), request body (truncated if large), response status code, response timing
  - Tool calls: tool name, arguments, result
  - Errors: full error chain with context
  - Spinner: start/stop events
  - Yolo: command being executed, exit code

## CLI Changes

```
-d, --debug    Enable debug logging to stderr
```

## Config File Changes

```toml
# config.toml
debug = true
```

## Env Var

```
CMDIFY_DEBUG=1
```

## Implementation Notes

- Add a `debug: bool` field to `Config` and `FileConfig` (following existing patterns)
- Add a `debug` flag to `Cli` struct
- Create a `src/debug.rs` module with:
  - A `static ENABLED: AtomicBool` gate
  - A `debug!()` macro that checks the gate before formatting/writing
  - A `debug_init(enabled: bool)` call in `main.rs` after config is loaded
- Instrument existing modules with `debug!()` calls at key points
- Mask API keys in any logged request headers: `Authorization: Bearer ***`
- Ensure debug output does not interfere with spinner output (stop spinner before writing debug, or write debug lines between spinner frames)

## Tests

- Unit: `debug!()` macro does nothing when disabled, writes to stderr when enabled
- Unit: `parse_bool_env` correctly parses `CMDIFY_DEBUG`
- Integration: `--debug` flag parsed correctly by CLI
- Integration: config file `debug = true` loaded correctly
- Integration: CLI > env var > config file precedence
- Verify: `cargo test` produces no debug output to stderr (debug is off by default)

## Acceptance Criteria

1. `cmdify -d "list files"` emits debug lines to stderr including config resolution and API request details
2. `cmdify "list files"` produces no debug output
3. `CMDIFY_DEBUG=1 cmdify "list files"` emits debug output
4. Setting `debug = true` in config.toml emits debug output
5. Debug output includes masked API keys, request timing, and response status
6. All existing tests pass; no debug output leaks in test runs
7. `make check` passes clean

## Out of Scope

- Log levels (trace/debug/info/warn/error hierarchy)
- Log file output (always stderr)
- Colored log output
- JSON-structured logging
