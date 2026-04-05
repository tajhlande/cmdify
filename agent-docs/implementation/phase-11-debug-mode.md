# Phase 11 — Debug Mode

## Goal

Add a debug/trace logging mode with two verbosity levels that emits diagnostic messages to stderr, giving users and developers visibility into cmdify's internal behavior — config resolution, API requests/responses, tool calls, and error details.

## Scope

- `-d` / `--debug` CLI flag with count semantics: `-d` (level 1), `-dd` (level 2), `--debug --debug` also works for scripts
- `CMDIFY_DEBUG` env var accepting `"0"`, `"1"`, `"2"`, `"true"`, `"false"`, `"yes"`, `"no"`
- `debug = true` option in `config.toml` maps to level 1
- Precedence: CLI and env/file debug levels are combined via `std::cmp::max` (highest wins)
- Debug output goes to stderr only, never stdout (stdout is reserved for the generated command)
- Zero-cost when disabled (no allocations or I/O unless debug is active)

## Design Decisions

- **No external logging crate** — lightweight internal macros behind a `static AtomicU8` gate. `debug!` macro (level 1), `debug_json!` macro (level 2 only). Zero allocations when level is 0.
- **Two verbosity levels**:
  - Level 1 (`-d`, `CMDIFY_DEBUG=1` or `true`): config sources, request URL, response status/timing, tool call names, `shell exec:` lines, errors (including raw body on decode failure)
  - Level 2 (`-dd`, `CMDIFY_DEBUG=2`): everything in level 1, plus full request/response JSON bodies via `serde_json::to_string_pretty`
- **Output format**: `DEBUG +<elapsed_ms>ms | <message>`. JSON content spans multiple lines. Every shell command is logged with `shell exec:` prefix for easy grepping.
- **Config source tracking**: `ConfigSource` struct records `key`, `value`, and `source` (env/file/cli) for every non-default config value. API keys masked as `***`.
- **Reserved extension points**: `emit_line_at(min_level, msg)` and `emit_json_at(min_level, label, value)` for future level 3+ if needed.
- **Testability**: `reset_for_test()`, `force_enable_for_test()`, `force_level_for_test(lvl)` helpers. File-based stderr capture with Mutex lock for parallel safety.

## CLI Changes

```
-d, --debug    Enable debug logging (-d basic, -dd verbose with JSON bodies)
                Uses clap ArgAction::Count so -dd gives level 2
```

## Config File Changes

```toml
# config.toml — maps to level 1
debug = true
```

## Env Var

```
CMDIFY_DEBUG=0    # disabled
CMDIFY_DEBUG=1    # basic (same as "true" or "yes")
CMDIFY_DEBUG=2    # verbose with JSON bodies
```

## Implementation

### `src/debug.rs`
- `LEVEL: AtomicU8` — global debug level (0, 1, or 2)
- `START_TIME: OnceLock<Instant>` — set on first enable for elapsed timing
- `init(level)` — called from `main.rs` after config resolution
- `emit_line(msg)` — level 1 gate, writes `eprintln!` with `DEBUG +Nms | msg`
- `emit_json(label, value)` — level 2 gate, writes pretty-printed JSON
- `emit_line_at(min_level, msg)` / `emit_json_at(min_level, label, value)` — reserved for future levels
- `debug!` macro → `emit_line` (level 1)
- `debug_json!` macro → `emit_json` (level 2)
- `format_line(msg)` / `format_json_line(label, value)` — pure formatting for testing

### `src/cli.rs`
- `-d` / `--debug` with `ArgAction::Count`, produces `u8` (0, 1, 2)

### `src/config.rs`
- `debug_level: u8` field on `Config`
- `parse_debug_env()` function: `"0"`/`"false"`/`"no"` → 0, `"1"`/`"true"`/`"yes"` → 1, `"2"` → 2
- `ConfigSource { key, value, source }` struct for tracking non-default config origins
- `from_env()` returns `(Config, Vec<ConfigSource>)` for main.rs source emission
- `ProviderSettings::from_env()` accepts `&mut Vec<ConfigSource>` for provider-specific source tracking

### `src/main.rs`
- CLI flags push `ConfigSource` entries with `source: "cli"`
- `config.debug_level = std::cmp::max(config.debug_level, cli.debug)` — merge via max
- After `debug::init()`, all config sources emitted via `debug!("Config: {} = {} ({})", ...)`
- Yolo command execution logged with `debug!("shell exec: {} -c {}", shell, content)`

### `src/provider/completions.rs`
- Level 1: request URL, response status + timing, errors, raw body on decode failure
- Level 2: full request body JSON, full response body JSON, error response JSON
- API key is set as HTTP header (not in JSON body), so it never appears in debug output

### `src/orchestrator.rs`
- Level 1: provider name, registered tools, loop iteration count, tool call names/args, tool results

### `src/tools/find_command.rs`
- Level 1: `shell exec: command -v <cmd>` and `shell exec: which <cmd>` for every lookup

## Tests

### Unit tests (`src/debug.rs`)
- Gate behavior: emit disabled when level=0, emit enabled when level>=1
- Format: `format_line` contains `DEBUG +Nms | msg`, `format_json_line` spans multiple lines
- Level gating: `emit_json` silent at level 1, active at level 2
- Stderr capture: file-based fd redirection with Mutex lock confirms actual stderr output
- Macro expansion: `debug!` and `debug_json!` macros write to stderr

### Unit tests (`src/config.rs`)
- `CMDIFY_DEBUG=0/false/no` → level 0 (source omitted)
- `CMDIFY_DEBUG=1/true/yes` → level 1 (source tracked as "env", value "1")
- `CMDIFY_DEBUG=2` → level 2 (source tracked as "env", value "2")
- `CMDIFY_DEBUG=3` → invalid, falls back to level 0
- `debug = true` in config.toml → level 1 (source tracked as "file", value "1")
- Precedence: `std::cmp::max(env_level, cli_level)` behavior documented
- API key masking: source value is `"***"` never the real key

### Integration tests
- `tools_test.rs`: timeout test with zero-second timeout returns `"error: command lookup timed out"`
- `provider_test.rs`: tools array correctly serialized in request body (custom wiremock matcher); no tools key when array empty
- `orchestrator_test.rs`: max iterations (10) exceeded error when provider always returns tool_calls

## Acceptance Criteria

1. `cmdify -d "list files"` emits level 1 debug lines to stderr
2. `cmdify -dd "list files"` emits level 2 debug lines including JSON request/response bodies
3. `cmdify "list files"` produces no debug output
4. `CMDIFY_DEBUG=1 cmdify "list files"` emits level 1 debug output
5. `CMDIFY_DEBUG=2 cmdify "list files"` emits level 2 debug output
6. Setting `debug = true` in config.toml emits level 1 debug output
7. Debug output includes config source tracking with masked API keys (`***`)
8. Every shell command execution is logged with `shell exec:` prefix
9. `grep 'shell exec:'` on stderr output finds all shell commands
10. All existing tests pass; no debug output leaks in test runs
11. Debug disabled has zero runtime cost (AtomicU8 check)

## Out of Scope

- Additional log levels beyond 2 (reserved via `emit_line_at`/`emit_json_at`)
- Log file output (always stderr)
- Colored log output
- JSON-structured logging
