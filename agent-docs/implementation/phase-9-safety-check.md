# Phase 9 — Safety Check

## Goal

Add a safety layer that inspects generated commands for potentially dangerous patterns before outputting or executing them. By default, unsafe commands are blocked with an error message; users must opt in via `--unsafe` / `-u`.

## Scope

- New `src/safety.rs` module with pattern-based unsafe command detection
- New `-u` / `--unsafe` CLI flag on `Cli`
- Safety check runs in `main.rs` after the orchestrator returns a command and before printing/executing it
- Error message to stderr with instructions on how to run in unsafe mode
- Config file support for `unsafe` setting

## Unsafe Patterns

The safety module should flag commands that contain patterns commonly associated with destructive or irreversible operations:

| Category | Patterns | Examples |
|----------|----------|---------|
| **Recursive delete** | `rm -rf /`, `rm -rf ~`, `rm -rf *` | `rm -rf /home`, `rm -rf ./*` |
| **Disk destruction** | `mkfs`, `dd` to disk devices | `dd if=/dev/zero of=/dev/sda` |
| **System-level changes** | `shutdown`, `reboot`, `init`, `systemctl poweroff` | `shutdown -h now`, `reboot` |
| **Kernel/module manipulation** | `modprobe -r`, `rmmod` of critical modules | `rmmod ext4` |
| **Partition manipulation** | `fdisk`, `parted`, `mkfs` on devices | `fdisk /dev/sda`, `mkfs.ext4 /dev/sdb1` |
| **Privilege escalation writes** | writing to `/etc/sudoers`, `/etc/passwd`, `/etc/shadow` | `echo >> /etc/passwd` |
| **Force kill all processes** | `kill -9 -1`, `pkill -9 -u root` | `kill -9 1` |
| **Package removal** | `apt remove --purge`, `dpkg --remove`, `yum remove`, `brew uninstall` | `apt remove --purge nginx` |

The pattern list should be stored as a static array of compiled regexes or glob patterns. The initial list should be conservative (more false positives than false negatives) and evolve over time.

## Behavior

```
if !cli.unsafe && safety::is_unsafe(&command):
    eprintln!("error: this command appears to be unsafe")
    eprintln!("  the command matched pattern: {matched_pattern}")
    eprintln!("  rerun with --unsafe (-u) to allow unsafe commands")
    exit 1
```

When `--unsafe` is passed, the safety check is skipped entirely and the command proceeds as normal (print and optionally execute via `--yolo`).

## Files to Create / Modify

```
src/
├── safety.rs          # CREATE: unsafe pattern detection
├── cli.rs             # MODIFY: add -u/--unsafe flag
├── main.rs            # MODIFY: wire safety check before output
└── lib.rs             # MODIFY: add safety module
```

## Implementation Steps

### 9.1 Safety module (`src/safety.rs`)

```rust
pub fn check(command: &str) -> Option<UnsafeMatch>

pub struct UnsafeMatch {
    pub pattern: &'static str,       // human-readable pattern name
    pub matched_text: String,         // the substring that matched
}
```

- `check()` iterates through the pattern list and returns the first match, or `None` if safe.
- Patterns should be matched against the trimmed, lowercased command string.
- Use simple substring or regex matching — no need for AST parsing of shell commands.

### 9.2 CLI flag (`cli.rs`)

Add to `Cli` struct:

```rust
#[arg(short = 'u', long = "unsafe", help = "Allow potentially unsafe commands")]
pub unsafe_mode: bool,
```

### 9.3 Wire into main.rs

After the orchestrator returns a command, before printing:

```rust
match result {
    Ok(content) => {
        if !cli.unsafe_mode {
            if let Some(match_) = safety::check(&content) {
                eprintln!("error: this command appears to be unsafe");
                eprintln!("  matched pattern: {}", match_.pattern);
                eprintln!("  rerun with --unsafe (-u) to allow");
                std::process::exit(1);
            }
        }
        println!("{}", content);
        // ... yolo logic
    }
    Err(e) => { ... }
}
```

### 9.4 Config file support

Add `unsafe` field to `FileConfig`:

```rust
struct FileConfig {
    // ... existing fields
    unsafe: Option<bool>,
}
```

Precedence: CLI flag > env var > config file > default (false).
Env var: `CMDIFY_UNSAFE=1`.

## Tests

**Unit tests for `safety.rs`:**
- Each unsafe pattern has a positive test (pattern matches)
- Safe commands pass through without matching
- Edge cases: commands with variable expansion, subshells, pipes

**CLI tests:**
- `-u` flag parses correctly
- `--unsafe` flag parses correctly

**Integration:**
- Safe command prints normally
- Unsafe command without `-u` exits 1 with error message
- Unsafe command with `-u` prints and executes normally

## Acceptance Criteria

- [ ] `cmdify "delete all files"` produces a command but blocks it with safety error
- [ ] `cmdify -u "delete all files"` allows the command through
- [ ] `cmdify "list files"` works normally without `-u`
- [ ] Error message includes the matched pattern and instructions
- [ ] `CMDIFY_UNSAFE=1` in config file enables unsafe mode
- [ ] `make check` passes
