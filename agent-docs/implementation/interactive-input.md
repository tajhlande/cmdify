# Interactive Input Mode

## Overview

The `-i` / `--interactive` flag allows users to enter command descriptions via an interactive prompt instead of positional CLI arguments. This avoids shell parsing issues with special characters (quotes, backticks, pipes, variable expansion, etc.) since the input is read as a raw string by `rustyline` rather than parsed by the shell.

## Files Changed

| File | Change |
|------|--------|
| `Cargo.toml` | Added `rustyline = { version = "18", features = ["with-file-history"] }` |
| `src/cli.rs` | Added `interactive` field with `conflicts_with_all = ["prompt", "setup"]` |
| `src/main.rs` | Replaced bare `read_line()` with rustyline-based `read_interactive_input()`; added history saving for CLI mode |
| `src/history.rs` | New module: XDG cache path resolution, append-with-trim, rustyline integration |
| `src/lib.rs` | Registered `pub mod history` |

## CLI Flag

```
-i, --interactive    Prompt for command description interactively (avoids shell parsing issues)
```

**Conflicts with:**
- Positional arguments (`prompt`) — clap rejects `cmdify -i find files` at parse time
- `--setup` — clap rejects `cmdify -i --setup` at parse time

## Interactive Input Flow

1. If stdin is not a TTY, print error and exit with code 1
2. Create a `rustyline::DefaultEditor` with `with-file-history` feature
3. Set max history size to 10,000 entries via `Configurer::set_max_history_size()`
4. Load existing history from `$XDG_CACHE_HOME/cmdify/history.txt` (or `$HOME/.cache/cmdify/history.txt`)
5. Print "Enter command description" to stderr
6. Display `> ` prompt via `rl.readline()`
7. Handle results:
   - **Valid input:** Trim whitespace, add to rustyline history, save history file, return the input
   - **Empty input:** Print help message (same as `cmdify` with no args) and exit with code 0
   - **Ctrl-C / Ctrl-D:** Print help message and exit with code 0
   - **Other error:** Print help message and exit with code 0

## Command History

All user prompts are persisted to a shared history file regardless of input mode.

### Location

```
$XDG_CACHE_HOME/cmdify/history.txt
```

Falls back to `$HOME/.cache/cmdify/history.txt` if `XDG_CACHE_HOME` is not set. Falls back to `cmdify_history.txt` in the current working directory if neither env var is set.

### Capacity

- **Maximum:** 10,000 lines
- **Trimming:** FIFO — when the limit is exceeded, the oldest entries are removed
- **CLI path:** `history::append_to_history()` appends the prompt, then calls `trim_history_if_needed()` which reads the file, counts lines, and rewrites with only the newest 10,000 entries if over limit
- **Interactive path:** rustyline's `set_max_history_size(10000)` caps in-memory history; `save_history()` only writes what's in memory, so the file naturally stays within limits

### Both Modes Share One File

- CLI invocations append directly via `history::append_to_history()`
- Interactive invocations use rustyline's `load_history()` / `add_history_entry()` / `save_history()`
- Both write to the same file path, so interactive up-arrow history includes prompts entered via CLI

### Graceful Degradation

If the history file cannot be created or written (permissions, no HOME, disk full), all errors are silently ignored. cmdify continues to function normally.

## Execution Order

The interactive input is read **before** config loading and validation (`Config::from_env()`). This means if config is broken, the user has already typed their input before seeing the error. This was a deliberate tradeoff — the alternative (validating config first, then prompting) would require a more complex control flow and the common case is that config is valid.

## Testing

### Unit Tests (`src/cli.rs`)

- `parse_interactive_short` — `-i` parsed correctly
- `parse_interactive_long` — `--interactive` parsed correctly
- `interactive_default_false` — default is `false` without flag
- `help_includes_interactive` — help text contains "interactive"
- `interactive_conflicts_with_prompt` — `cmdify -i find files` rejected by clap
- `interactive_conflicts_with_setup` — `cmdify -i --setup` rejected by clap

### Unit Tests (`src/history.rs`)

- `history_path_with_xdg_cache_home` — resolves `$XDG_CACHE_HOME/cmdify/history.txt`
- `history_path_with_home_fallback` — resolves `$HOME/.cache/cmdify/history.txt`
- `history_path_no_xdg_no_home` — falls back to `cmdify_history.txt`
- `append_creates_file_and_directory` — creates cache dir and file
- `append_multiple_entries` — appends in order, one per line
- `xdg_takes_precedence_over_home` — XDG env var wins over HOME
- `trim_trims_oldest_entries_at_limit` — at 10,001 entries, oldest is removed
- `trim_not_triggered_below_limit` — 50 entries left untouched
- `max_history_lines_is_10000` — constant is correct

### Integration Tests (`tests/cli_test.rs`)

- `parse_interactive_short` — integration-level parse check
- `parse_interactive_long` — integration-level parse check
- `interactive_default_false` — integration-level default check
- `help_contains_interactive` — integration-level help check

## Remaining Assumptions

1. **Single-line input only** — `rustyline`'s `readline()` reads one line. Multi-line paste only captures the first line. We rely on terminal bracketed paste behavior rather than handling this in cmdify.

2. **Interactive check before config validation** — The user types their input before config is validated. If config is broken, the input is wasted. Accepted as a reasonable tradeoff.

3. **History trim is read-rewrite on CLI path** — `trim_history_if_needed()` reads the entire file into memory, checks line count, and rewrites if over limit. This is fine for a 10,000-line text file but could be slow on very constrained systems.

4. **No history dedup for CLI path** — CLI mode appends every invocation to the file, even if the same prompt was just entered. rustyline handles dedup within interactive sessions, but CLI→interactive cross-session dedup is not implemented.

5. **History file format is line-oriented** — Each line is one history entry. Prompts containing literal newlines would break the format. This is unlikely given that prompts are single-line descriptions of shell commands.
