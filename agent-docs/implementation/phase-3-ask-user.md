# Phase 3 — `ask_user` Tool

## Goal

Add the `ask_user` tool, enabling the model to ask clarifying questions when the user's request is ambiguous. This completes the tool system.

## Scope

- `ask_user` tool implementation (interactive stdin/stderr)
- Wire up `-q` / `--quiet` flag (disables `ask_user`)
- Full CLI flag combinations now functional
- System prompt update mentioning both tools
- System prompt instructions for when to use each tool

## Files to Create / Modify

```
src/
├── system_prompt.txt       # MODIFY: mention both tools
├── orchestrator.rs         # MODIFY: (minor) ensure ToolRegistry gets both tools
└── tools/
    ├── mod.rs              # MODIFY: register AskUserTool
    └── ask_user.rs         # CREATE: ask_user tool
tests/
└── tools_test.rs           # MODIFY: add ask_user tests
```

## Implementation Steps

### 3.1 `ask_user` tool (`src/tools/ask_user.rs`)

Per `TOOLS.md §4.1`:

- Receives `question` (string) and `choices` (array of `{ key, label }` objects)
- Prints to **stderr** (not stdout):
  ```
  > [cmdify] Use fd or find?
    A) use fd
    B) use find
  > Your choice:
  ```
- Reads a single line from stdin (blocking, via `tokio::task::spawn_blocking`)
- 60-second timeout: returns `ToolOutput { content: "(no response)" }` on timeout
- Invalid input: returns `ToolOutput { content: "X (not a valid choice)" }`
- Valid input: returns `ToolOutput { content: "A" }` (just the key)

### 3.2 Tool registry update (`src/tools/mod.rs`)

Update `ToolRegistry::new()` to register both tools:

| Flag | Tools registered |
|------|-----------------|
| (none) | `ask_user`, `find_command` |
| `-q` | `find_command` only |
| `-b` | `ask_user` only |
| `-n` | Empty — no tools |

### 3.3 CLI flag conflicts

Ensure `clap` is configured so `-n` conflicts with `-q` and `-b` (per `DESIGN.md §4.1`).

### 3.4 System prompt update

Update `src/system_prompt.txt` to:
- Describe both tools and when to use them
- `ask_user`: use when the request is ambiguous and needs clarification
- `find_command`: use to verify commands exist before suggesting them
- Instruct model to prefer using tools over guessing

## Tests

**Unit tests:**
- `tools/ask_user.rs`: parse valid choice key, parse invalid input, timeout handling, choice formatting
- `tools/mod.rs`: registry with `-q` excludes `ask_user`, with `-b` excludes `find_command`, with `-n` is empty

**Integration tests:**
- `tools_test.rs`: mocked stdin providing valid and invalid responses
- `orchestrator_test.rs`: mock provider asks question → user answers → mock provider returns final command

## Acceptance Criteria

- [ ] Model can ask clarifying questions via `ask_user`
- [ ] `-q` / `--quiet` disables `ask_user` but keeps `find_command`
- [ ] `-b` / `--blind` disables `find_command` but keeps `ask_user`
- [ ] `-n` / `--no-tools` disables all tools
- [ ] `-n` conflicts with `-q` and `-b` in clap
- [ ] Interactive prompts go to stderr, not stdout
- [ ] Pipe usage works: `$(cmdify list files)` captures only the final command
- [ ] `make check` passes
