# Phase 4 — Tool Levels

## Goal

Introduce a numbered tool level system that provides progressive disclosure of environment awareness tools. This phase adds the infrastructure (CLI flag, config, registry refactoring, `--list-tools` help text) without adding new tools. Existing tools are assigned to levels, and legacy flags (`-q`/`-b`/`-n`) are preserved as overrides.

## Scope

- `-t N` / `--tools N` CLI flag (0–3)
- `CMDIFY_TOOL_LEVEL` env var
- `tool_level` field in `config.toml`
- `--list-tools` flag that prints available tools by level and exits
- Refactor `ToolRegistry::new()` to use level-based selection
- Preserve `-q`/`-b`/`-n` as overrides layered on top of tool level
- Updated help text referencing tool levels

## Tool Level Definitions

| Level | Name | Tools | Risk Profile |
|-------|------|-------|-------------|
| 0 | none | *(no tools)* | No tool access. Model generates commands from context only. |
| 1 | core | `ask_user`, `find_command`, `list_current_directory` | Interactive clarification, command existence checks, cwd file listing. Read-only, no filesystem writes, no system introspection beyond PATH. |
| 2 | local | `command_help`, `list_any_directory`, `pwd` | Read-only filesystem access (help text with optional grep filter, arbitrary directory listing, working directory). Still no writes or system introspection. |
| 3 | system | `get_env`, `list_processes` | System introspection (environment variables, running processes). Highest trust level — model can observe system state beyond the filesystem. |

**Default:** Level 1. This provides a conservative risk profile while still being useful.

### Risk philosophy

Each level grants the model additional information about the user's environment. More information improves command accuracy but increases the attack surface if the model is malicious or the user is tricked into running cmdify with a compromised prompt. The level system lets users choose their comfort level:

- **Level 0** — Maximum privacy. The model sees nothing beyond the user's prompt.
- **Level 1** — Safe defaults. The model can ask questions, verify commands exist, and see what files are in the current directory. It cannot read arbitrary paths, see environment variables, or inspect processes.
- **Level 2** — Extended filesystem awareness. The model can read help text filtered by grep patterns (reducing incorrect flag usage), list any directory (useful for "compress my project" type requests), and confirm the working directory. Still no system introspection.
- **Level 3** — Full environment awareness. The model can read environment variables (potentially revealing secrets) and list running processes. Use with caution in sensitive environments.

### Future tools

New tools should be assigned to a level based on their risk profile, not their usefulness. A tool that reads arbitrary files from disk would be level 2 (filesystem access). A tool that writes files would require a new level 4 or remain unimplemented. The level system is extensible but should remain small (ideally 0–4).

## Configuration

**Precedence:** CLI flag > env var > config file > default (1)

| Source | Example |
|--------|---------|
| CLI flag | `cmdify -t 2 "compress my project"` |
| Env var | `CMDIFY_TOOL_LEVEL=2` |
| Config file | `tool_level = 2` |
| Default | `1` (if none specified) |

### Legacy flag interaction

The existing `-q`, `-b`, and `-n` flags are preserved as convenience overrides layered on top of the tool level:

| Combination | Effect |
|-------------|--------|
| `-t 1` | `ask_user`, `find_command`, `list_current_directory` |
| `-t 1 -q` | `find_command`, `list_current_directory` (removes `ask_user`) |
| `-t 2 -b` | Level 2 tools minus `find_command` |
| `-t 3 -n` | No tools (`-n` is absolute) |
| `-n` | No tools regardless of `-t` |
| `-q -b` (no `-t`) | Level 1 minus both `ask_user` and `find_command` |

Implementation: `ToolRegistry::new(level, quiet, blind, no_tools)` — `no_tools` is checked first (absolute), then `level` determines the base set, then `quiet`/`blind` remove individual tools.

## `--list-tools` Output

Running `cmdify --list-tools` prints the following to stdout and exits with code 0:

```
cmdify tool levels (default: 1)

Level 0 — no tools

Level 1 — core:
  ask_user                Ask the user a clarifying question
  find_command            Check whether a command exists on the system
  list_current_directory  List files in the current working directory

Level 2 — local (not yet implemented):
  command_help            Show help text for a command (optional grep filter)
  list_any_directory      List files in any user-specified directory
  pwd                     Print the current working directory

Level 3 — system (not yet implemented):
  get_env                 Read environment variables
  list_processes          List running processes

Use -t N or --tools N to set the tool level (0-3).
Use -q, -b, -n to disable individual tools or all tools.
```

Tools not yet implemented are shown at their planned level so users know what's coming. Once implemented, the "(not yet implemented)" label is removed.

## Files to Create / Modify

```
src/
├── cli.rs               # MODIFY: add -t/--tools, --list-tools
├── config.rs            # MODIFY: add tool_level field + env var + config resolution
├── main.rs              # MODIFY: wire --list-tools, pass tool_level to registry
├── tools/
│   └── mod.rs           # MODIFY: refactor ToolRegistry::new() to use levels
└── orchestrator.rs      # MODIFY: pass tool_level instead of quiet/blind to registry
```

## Implementation Steps

### 4.1 Config (`src/config.rs`)

Add `tool_level` field:

```rust
pub struct Config {
    // ... existing fields
    pub tool_level: u8,
}
```

Resolution logic (same pattern as other config):

```rust
let tool_level = resolve_u8(
    "CMDIFY_TOOL_LEVEL",
    file_config.as_ref().and_then(|f| f.tool_level),
    &mut sources,
    "tool_level",
).unwrap_or(1);
```

Clamp to valid range: `tool_level.min(3)`.

### 4.2 CLI (`src/cli.rs`)

```rust
#[arg(
    short = 't',
    long = "tools",
    value_name = "N",
    help = "Tool level: 0 (none), 1 (core, default), 2 (local), 3 (system)"
)]
pub tool_level: Option<u8>,

#[arg(long = "list-tools", help = "List all available tools by level and exit")]
pub list_tools: bool,
```

### 4.3 `--list-tools` in `main.rs`

Before the orchestrator runs, check for `--list-tools`:

```rust
if cli.list_tools {
    print_tool_levels();
    std::process::exit(0);
}
```

The `print_tool_levels()` function prints the formatted output to stdout. Tool descriptions come from each tool's `ToolDefinition`.

### 4.4 ToolRegistry refactor (`src/tools/mod.rs`)

Change the registry constructor:

```rust
impl ToolRegistry {
    pub fn new(tool_level: u8, quiet: bool, blind: bool, no_tools: bool) -> Self {
        if no_tools {
            return Self { tools: Vec::new() };
        }

        let mut tools: Vec<Box<dyn Tool>> = Vec::new();

        // Level 1 tools
        if tool_level >= 1 {
            if !quiet {
                tools.push(Box::new(AskUserTool::default()));
            }
            if !blind {
                tools.push(Box::new(FindCommandTool::default()));
            }
            // list_current_directory will be added here when implemented
        }

        // Level 2 tools (not yet implemented)
        // Level 3 tools (not yet implemented)

        Self { tools }
    }
}
```

### 4.5 Orchestrator update (`src/orchestrator.rs`)

```rust
let registry = ToolRegistry::new(config.tool_level, config.quiet, config.blind, config.no_tools);
```

### 4.6 CLI override in `main.rs`

```rust
config.tool_level = cli.tool_level.unwrap_or(config.tool_level);
config.tool_level = config.tool_level.min(3);
```

## Tests

**Unit tests (`src/cli.rs`):**
- `-t 0` parses as `Some(0)`
- `-t 2` parses as `Some(2)`
- `--tools 3` parses as `Some(3)`
- `--list-tools` parses as `true`
- Default is `None` (falls through to config)

**Unit tests (`src/config.rs`):**
- `CMDIFY_TOOL_LEVEL=2` → `config.tool_level == 2`
- `tool_level = 3` in config.toml → `config.tool_level == 3`
- CLI override takes precedence over env var
- Invalid value (e.g., `"abc"`) falls back to default (1)
- Value clamped to 3

**Unit tests (`src/tools/mod.rs`):**
- Level 0 → empty registry
- Level 1 → `ask_user` + `find_command`
- Level 1 + `-q` → `find_command` only
- Level 2 → same as level 1 (no new tools yet)
- Level 2 + `-b` → `ask_user` only (no `find_command`)
- Level 3 + `-n` → empty registry

**Integration tests:**
- `--list-tools` exits 0 and prints level descriptions to stdout

## Acceptance Criteria

- [ ] `cmdify --list-tools` prints tool levels to stdout and exits 0
- [ ] `-t 0` disables all tools
- [ ] `-t 1` (or no `-t`) enables core tools
- [ ] `-t 2` enables core tools (no new tools yet, but infrastructure ready)
- [ ] `-q` removes `ask_user` from any level
- [ ] `-b` removes `find_command` from any level
- [ ] `-n` overrides all levels (no tools)
- [ ] `CMDIFY_TOOL_LEVEL=2` works via env var
- [ ] `tool_level = 2` works in config.toml
- [ ] CLI flag overrides env var and config file
- [ ] `make check` passes
