# Phase 5 — Safety Check

## Goal

Add a three-layer safety system that protects users from destructive commands. The layers work together: the LLM is guided to avoid dangerous output via modular system prompt instructions, the model can use `ask_user` for confirmation when uncertainty exists, and a deterministic semantic checker acts as a hard backstop that cannot be bypassed.

## Scope

- Split the monolithic system prompt into modular pieces assembled at runtime
- Add LLM self-classification guidance to the system prompt
- New `src/safety.rs` module with tokenize-then-analyze semantic checks
- Wire safety check into `main.rs` after the orchestrator returns a command
- Error messages with differentiated context (user confirmed vs not)
- Add `shlex` dependency for shell tokenization

### Out of scope

- The `-u` / `--unsafe` CLI flag, env var, and config field are already implemented (`config.allow_unsafe`)
- Pattern-based regex matching (replaced by semantic checks)

## Safety Architecture

```
Layer 1 — LLM Guidance (system prompt)
    "Before outputting a command, consider its safety..."
    → Model uses ask_user for confirmation, prefers safe alternatives
    → Advisory only — can be bypassed by adversarial prompts or model errors

Layer 2 — Deterministic Semantic Check (safety.rs)
    Tokenize command → multi-pass analysis → block or allow
    → Hard gate — cannot be talked around
    → Runs after the model produces output, before printing/executing

Layer 3 — User Opt-Out (--unsafe)
    When allow_unsafe is true, both layers are skipped
    → Explicit escape hatch for experienced users
```

The layers are defense-in-depth. The LLM layer reduces how often dangerous commands are generated, making the experience smoother. The semantic check catches what the LLM misses. Neither is sufficient alone.

---

## Part A — Modular System Prompt

### Current problem

The system prompt (`src/system_prompt.txt`) is monolithic — it always includes tool guidance even when `-n` is set, and never mentions safety. It doesn't adapt to the runtime configuration.

### New file structure

Split `system_prompt.txt` into four pieces, each compiled in via `include_str!()`:

| File | Content | When included |
|------|---------|--------------|
| `system_prompt_base.txt` | Identity, output format, shell compatibility rules | Always |
| `system_prompt_tools.txt` | Tool usage guidance — use ask_user for ambiguity, use find_command to verify | When `!no_tools` |
| `system_prompt_safety.txt` | Self-classification instructions, ask_user confirmation, safe alternatives | When `!allow_unsafe` |
| `system_prompt_unsafe.txt` | Brief note: "Unsafe mode — no safety restrictions on generated commands" | When `allow_unsafe` |

### Prompt assembly in `src/prompt.rs`

```rust
pub const PROMPT_BASE: &str = include_str!("system_prompt_base.txt");
pub const PROMPT_TOOLS: &str = include_str!("system_prompt_tools.txt");
pub const PROMPT_SAFETY: &str = include_str!("system_prompt_safety.txt");
pub const PROMPT_UNSAFE: &str = include_str!("system_prompt_unsafe.txt");

pub fn load_system_prompt(config: &Config) -> Result<String> {
    let base = if let Some(ref path) = config.system_prompt_override {
        std::fs::read_to_string(path)?
    } else {
        PROMPT_BASE.to_string()
    };

    let mut parts = vec![base];

    if !config.no_tools {
        parts.push(PROMPT_TOOLS.to_string());
    }

    if config.allow_unsafe {
        parts.push(PROMPT_UNSAFE.to_string());
    } else {
        parts.push(PROMPT_SAFETY.to_string());
    }

    let shell = env::var("SHELL")
        .ok()
        .and_then(|s| s.rsplit('/').next().map(|n| n.to_string()))
        .unwrap_or_else(|| "bash".to_string());

    parts.push(format!("The user's shell is {}.", shell));

    Ok(parts.join("\n\n"))
}
```

### Content of each prompt piece

#### `system_prompt_base.txt`

```
You are a shell command generator. Given a natural language description, respond with a single shell command that accomplishes the requested task.

Rules:
- Output ONLY the command, nothing else
- No markdown fences, no explanation, no commentary
- The command must be compatible with bash and zsh
- Use common, well-known commands and flags
- Prefer simple, readable commands over complex ones
```

(Unchanged from current lines 1-8.)

#### `system_prompt_tools.txt`

```
Tool usage:
- Use the ask_user tool when the request is ambiguous and needs clarification from the user. For example, if multiple commands could achieve the same goal, ask which one the user prefers.
- Use the find_command tool to verify that suggested commands exist on the user's system before outputting them. If a command might not be installed, check first.
- Prefer using tools over guessing. If you are unsure, use the tools to gather information before producing a final answer.
```

(Unchanged from current lines 10-13. Only included when tools are registered.)

#### `system_prompt_safety.txt`

```
Safety:
- Before outputting a command, consider whether it could cause irreversible harm: data loss, system instability, or security exposure.
- If the command is potentially dangerous, use the ask_user tool to confirm with the user before outputting it. Present the risk clearly in the question.
- If no tools are available (ask_user is disabled), prefer safe alternatives: dry-run flags (-n, --dry-run), interactive confirmation flags (-i, -I), or scoped targets (specific paths instead of broad wildcards).
- When in doubt about safety, choose the safer option.
```

Key design choices:
- No structured output marker (e.g., `[UNSAFE]`) — a marker could be spoofed by adversarial prompts and doesn't add value over the hard semantic check
- The prompt is aware that tools may or may not be available — it instructs the model to adapt its strategy accordingly
- "When in doubt" clause gives the model permission to be conservative

#### `system_prompt_unsafe.txt`

```
Unsafe mode is active. No safety restrictions on generated commands — output whatever command best matches the user's request, even if it is destructive.
```

Minimal. No need for elaboration — the user explicitly opted in.

### Runtime modes

| Config | Tools | Safety prompt piece |
|--------|-------|-------------------|
| Default | Enabled | `system_prompt_safety.txt` (model asks_user, prefers safe alternatives) |
| `-n` | Disabled | `system_prompt_safety.txt` (model prefers safe alternatives, no ask_user) |
| `--unsafe` | Enabled | `system_prompt_unsafe.txt` (no restrictions) |
| `-n --unsafe` | Disabled | `system_prompt_unsafe.txt` (no restrictions) |

### Tests for prompt modularization

- Each prompt piece file is non-empty
- Assembly with default config includes base + tools + safety + shell
- Assembly with `-n` excludes tools piece
- Assembly with `--unsafe` includes unsafe piece, excludes safety piece
- Assembly with `-n --unsafe` includes only base + unsafe + shell
- Shell detection still works with all combinations
- Custom prompt override still takes precedence over all pieces

---

## Part B — Deterministic Semantic Safety Check

### Approach

Replace simple regex matching with shell tokenization followed by multi-pass structural analysis. This catches evasion techniques that regex misses (quoted splits, variable expansion, subshell injection).

### Dependency

Add `shlex` to `Cargo.toml`:

```toml
shlex = "1"
```

`shlex` is pure Rust with no native dependencies — it does not violate the zero-runtime-deps goal.

### Multi-pass analysis

```
Input: raw command string
  │
  ▼
Pass 1: Structural — reject command injection constructs
  │  Block: backticks, $(), pipes to sh/bash/zsh, eval, exec with substitution
  │  These are injection vectors regardless of what command they contain
  │
  ▼
Pass 2: Command-level — check the primary command name
  │  Match against danger list (rm, mkfs, dd, shutdown, reboot, fdisk, etc.)
  │  If safe command → allow
  │  If dangerous command → continue to pass 3
  │
  ▼
Pass 3: Flag-level — check for dangerous flags
  │  Match against flag patterns (-rf, --force, --no-preserve-root, etc.)
  │  If no dangerous flags → allow
  │  If dangerous flags → continue to pass 4
  │
  ▼
Pass 4: Target-level — check for broad/sensitive targets
  │  Match against target patterns (/, ~, *, /etc, /boot, /sys, /proc, /dev/sd*)
  │  If scoped target (e.g., /tmp/stale, ./build) → allow
  │  If broad target → BLOCK
  │
  ▼
Output: None (safe) or UnsafeMatch { pass, category, matched_text }
```

### Public API

```rust
pub fn check(command: &str) -> Option<UnsafeMatch>

pub struct UnsafeMatch {
    pub pass: u8,             // which pass blocked (1-4)
    pub category: &'static str, // human-readable category name
    pub matched_text: String,   // the specific text that triggered the match
}
```

### Pass 1: Structural checks

Reject commands that contain shell injection constructs regardless of the command name:

| Construct | Reason |
|-----------|--------|
| `` `...` `` | Command substitution via backticks |
| `$(...)` | Command substitution |
| `| sh`, `| bash`, `| zsh` | Piping to a shell (arbitrary execution) |
| `eval` | Evaluating arbitrary strings as commands |
| `exec` with substitution | Replacing process with arbitrary command |

Implementation: Use `shlex::split()` to tokenize, then scan tokens for these patterns. Also check the raw string for constructs that `shlex` might not expose (e.g., backticks inside quotes).

### Pass 2: Command-level checks

Dangerous command names (the first token after tokenization):

| Category | Commands |
|----------|----------|
| Disk/filesystem destruction | `mkfs`, `dd`, `fdisk`, `parted`, `mkswap` |
| System state changes | `shutdown`, `reboot`, `halt`, `poweroff`, `init` |
| Kernel manipulation | `modprobe`, `rmmod`, `insmod` |
| Privilege escalation writes | commands writing to `/etc/sudoers`, `/etc/passwd`, `/etc/shadow` |

### Pass 3: Flag-level checks

Dangerous flag combinations (checked only if the command was flagged in pass 2, or for any command with these flags):

| Command | Dangerous flags |
|---------|----------------|
| `rm` | `-rf`, `-fr`, `-r -f`, `--recursive --force`, `--no-preserve-root` |
| `chmod` | `-R 777`, `-R a+rw` |
| `kill` | `-9 -1`, `--signal 9 -1` |
| `mv` | with `/` or `~` as target |
| `cp` | with `/etc/` as target |

### Pass 4: Target-level checks

Broad or sensitive path targets:

| Pattern | Examples |
|---------|---------|
| Root filesystem | `/`, `/bin`, `/sbin`, `/lib` |
| System directories | `/etc`, `/boot`, `/sys`, `/proc`, `/dev` |
| Home directory root | `~`, `$HOME` (literal, not expanded) |
| Wildcard all | `*` as a standalone argument |
| Block devices | `/dev/sda`, `/dev/nvme`, `/dev/rdisk` |

### Behavior

```rust
match result {
    Ok(content) => {
        if !config.allow_unsafe {
            if let Some(match_) = safety::check(&content) {
                eprintln!("error: command blocked by safety check");
                eprintln!("  pass {}: {} — matched: \"{}\"",
                    match_.pass, match_.category, match_.matched_text);
                eprintln!("  rerun with --unsafe (-u) to allow unsafe commands");
                std::process::exit(1);
            }
        }
        println!("{}", content);
        // ... yolo logic
    }
    Err(e) => { ... }
}
```

### Why not use the conversation history?

The semantic checker could theoretically inspect the conversation to see if `ask_user` was used for confirmation. This is intentionally **not done** because:

1. It couples the safety module to the orchestrator's internal state
2. A user could craft a prompt that tricks the model into calling `ask_user` with a benign-looking question
3. The hard check should be independent and deterministic — same input always produces the same output
4. Users who genuinely want to run a dangerous command have `--unsafe`

---

## Files to Create / Modify

```
src/
├── safety.rs                      # CREATE: tokenize + multi-pass semantic checks
├── prompt.rs                      # MODIFY: modular prompt assembly
├── main.rs                        # MODIFY: wire safety check before output
├── lib.rs                         # MODIFY: add safety module
├── system_prompt_base.txt         # CREATE: base prompt (from current system_prompt.txt)
├── system_prompt_tools.txt        # CREATE: tool guidance piece
├── system_prompt_safety.txt       # CREATE: safety guidance piece
├── system_prompt_unsafe.txt       # CREATE: unsafe mode piece
└── system_prompt.txt              # DELETE: replaced by the four pieces above
Cargo.toml                         # MODIFY: add shlex dependency
```

## Implementation Steps

### 5.1 Add `shlex` dependency

```toml
shlex = "1"
```

### 5.2 Split system prompt

1. Create `system_prompt_base.txt` with lines 1-8 of the current prompt
2. Create `system_prompt_tools.txt` with lines 10-13 of the current prompt
3. Create `system_prompt_safety.txt` with the safety guidance text
4. Create `system_prompt_unsafe.txt` with the unsafe mode note
5. Delete the original `system_prompt.txt`

### 5.3 Refactor `prompt.rs`

- Replace single `EMBEDDED_SYSTEM_PROMPT` with four `include_str!()` constants
- Rewrite `load_system_prompt()` to assemble pieces based on `config.no_tools` and `config.allow_unsafe`
- Preserve backward compatibility: custom prompt override still takes precedence
- Update tests for all flag combinations

### 5.4 Implement `safety.rs`

Implement the four-pass analysis pipeline:

```rust
// Pass 1: Structural
fn check_structural(tokens: &[&str], raw: &str) -> Option<UnsafeMatch>

// Pass 2: Command-level
fn check_command(tokens: &[&str]) -> Option<UnsafeMatch>

// Pass 3: Flag-level
fn check_flags(command: &str, tokens: &[&str]) -> Option<UnsafeMatch>

// Pass 4: Target-level
fn check_targets(tokens: &[&str]) -> Option<UnsafeMatch>

// Main entry point
pub fn check(command: &str) -> Option<UnsafeMatch>
```

Tokenization via `shlex::split()`. Each pass function returns `None` (safe) or `Some(UnsafeMatch)` (blocked).

### 5.5 Wire into `main.rs`

Insert safety check between orchestrator output and command printing/executing (before the existing `--yolo` execution block).

### 5.6 Update `lib.rs`

Add `pub mod safety;` to the module declarations.

## Tests

**Prompt modularization tests (`src/prompt.rs`):**
- Each `include_str!` constant is non-empty
- Default assembly: base + tools + safety + shell
- With `-n`: base + safety + shell (no tools piece)
- With `--unsafe`: base + tools + unsafe + shell
- With `-n --unsafe`: base + unsafe + shell
- Custom override path still works and replaces all pieces
- Shell detection works with all combinations

**Safety module tests (`src/safety.rs`):**

Pass 1 — Structural:
- `` echo `whoami` `` → blocked (backtick substitution)
- `echo $(whoami)` → blocked (command substitution)
- `ls | sh` → blocked (pipe to shell)
- `eval "rm -rf /"` → blocked (eval)
- `ls | grep foo` → safe (pipe to non-shell)

Pass 2 — Command-level:
- `mkfs /dev/sda` → blocked
- `shutdown -h now` → blocked
- `ls -la` → safe
- `rm -rf /tmp/stale` → passes pass 2 (rm is flagged, continues to pass 3)

Pass 3 — Flag-level:
- `rm -rf /tmp/stale` → blocked (dangerous flags + any target)
- `rm -i /tmp/stale` → safe (interactive flag, no force)
- `rm -r /tmp/stale` → safe (recursive but not forced)

Pass 4 — Target-level:
- `rm -rf /tmp/stale` → blocked (dangerous flags, target is scoped but combination is risky)
- `rm -rf /` → blocked (broad target)
- `rm -rf ~` → blocked (home directory root)
- `rm -rf /etc/passwd` → blocked (sensitive path)
- `rm -rf ./build` → allowed after passes (scoped target)

Edge cases:
- `echo "rm -rf /"` → safe (not executed, just echoed)
- Variable expansion: `rm -rf $DIR` → blocked ($DIR is unexpanded, could be anything)
- Chained commands: `ls && rm -rf /` → blocked (second command triggers)
- Quoted path: `rm -rf "/tmp/my dir"` → blocked (dangerous flags present)

**Integration:**
- Safe command prints normally
- Unsafe command without `--unsafe` exits 1 with error
- Unsafe command with `--unsafe` prints normally
- Error message includes pass number, category, and matched text

## Acceptance Criteria

- [x] System prompt is split into four modular pieces
- [x] Default assembly includes base + tools + safety + shell
- [x] `-n` excludes tools piece from prompt
- [x] `--unsafe` replaces safety piece with unsafe piece
- [x] Custom prompt override still works
- [x] Safety check tokenizes commands via `shlex` before analysis
- [x] Pass 1 blocks shell injection constructs (backticks, $(), pipes to shell)
- [x] Pass 2 flags dangerous command names
- [x] Pass 3 flags dangerous flag combinations
- [x] Pass 4 flags broad/sensitive path targets
- [x] `echo "dangerous command"` passes (string literal, not executed)
- [x] Error message includes pass number, category, and matched text
- [x] `--unsafe` skips both LLM guidance and semantic check
- [x] `make check` passes
