# Phase 11 — Interactive Setup

## Goal

Provide a first-run setup experience so users can configure `cmdify` interactively without manually creating config files or setting environment variables. Also add a `--setup` flag to re-enter setup mode at any time.

## Scope

- New `src/setup.rs` module with interactive config wizard
- New `--setup` CLI flag
- First-run detection: check for `~/.config/cmdify/config.toml` on startup
- Auto-enter setup mode when no config file exists and terminal is interactive
- Non-interactive fallback message when no config file exists (unless `--quiet`/`-q`)
- `--setup` flag triggers setup mode (reads existing values as defaults)
- `--setup` on a non-interactive terminal exits with an error

## Behavior Matrix

| Condition | Terminal is interactive | Terminal is NOT interactive |
|-----------|----------------------|---------------------------|
| Config file exists, no `--setup` | Normal operation | Normal operation |
| Config file missing, no `--setup`, no `--quiet` | Enter setup mode | Print message to stderr, proceed (may fail on missing required env vars) |
| Config file missing, no `--setup`, `--quiet` | Normal operation (no message) | Normal operation (no message) |
| `--setup` flag, interactive | Enter setup mode (existing values as defaults) | Error: "setup requires an interactive terminal" |

## Setup Flow

```
1. Display: "cmdify setup — configure your default provider"
2. Prompt: "Select a provider (default: openai)"
   - List all 13 providers with numbers
   - Accept number or name
3. Prompt: "Model name (default: gpt-4o): "
   - Provider-specific suggestions shown as hints
4. Prompt: "Max tokens (default: 16384): "
   - Accept number or empty for default
5. Prompt: "Custom system prompt file (optional, leave blank for default): "
   - Accept path or empty
6. For providers requiring an API key, prompt:
   "API key for <provider> (will be stored as <ENV_VAR> in your shell profile):
   - Do NOT store the key in the config file
   - Print the export command the user should add to their shell profile:
     e.g., echo 'export OPENAI_API_KEY=sk-...' >> ~/.zshrc
7. Summary: print the config file path and contents
8. Write to ~/.config/cmdify/config.toml
9. Print: "Config written to ~/.config/cmdify/config.toml"
   Print: "Add the following to your shell profile:"
   Print: "  export <API_KEY_VAR>=<value>"
```

## Files to Create / Modify

```
src/
├── setup.rs          # CREATE: interactive config wizard
├── cli.rs            # MODIFY: add --setup flag
├── main.rs           # MODIFY: first-run detection, wire setup flow
└── lib.rs            # MODIFY: add setup module
```

## Implementation Steps

### 11.1 CLI flag (`cli.rs`)

```rust
#[arg(long = "setup", help = "Run interactive setup wizard")]
pub setup: bool,
```

Note: no short flag for `--setup` to avoid conflicts with `-s` (spinner).

### 11.2 Config directory detection

Add a public function to `config.rs`:

```rust
pub fn config_dir() -> PathBuf {
    if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg).join("cmdify")
    } else if let Ok(home) = env::var("HOME") {
        home.into(".config").join("cmdify")
    } else {
        PathBuf::from(".cmdify")
    }
}

pub fn config_file_path() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn config_exists() -> bool {
    config_file_path().exists()
}
```

The existing private `config_file_path()` function should be refactored to use this shared implementation.

### 11.3 Setup module (`src/setup.rs`)

```rust
pub fn run_interactive(existing_config: Option<&FileConfig>) -> Result<()>
```

- Reads existing config values as defaults if present
- Uses stdin/stderr for all prompts (stdout remains clean)
- Creates `~/.config/cmdify/` directory if it doesn't exist
- Writes `config.toml` with only the non-secret fields
- Prints shell export commands for API keys
- Does NOT store API keys in the config file

### 11.4 First-run detection in `main.rs`

Startup flow in `main()`:

```rust
let cli = Cli::parse();

// --setup flag
if cli.setup {
    if !std::io::stderr().is_terminal() {
        eprintln!("error: --setup requires an interactive terminal");
        std::process::exit(1);
    }
    let existing = load_existing_config_if_present();
    setup::run_interactive(existing)?;
    return;
}

// No prompt args — show help and exit (existing behavior)
if cli.prompt.is_empty() {
    Cli::parse_from(["cmdify", "--help"]);
    return;
}

// First-run detection
if !config::config_exists() && std::io::stderr().is_terminal() && !cli.quiet {
    let existing = None;
    setup::run_interactive(existing)?;
    // After setup, reload config and continue
}

// Non-interactive first-run: print a message unless quiet
if !config::config_exists() && !cli.quiet {
    eprintln!("hint: no config file found at {}. run 'cmdify --setup' to configure.", config::config_file_path().display());
}
```

### 11.5 Provider-specific suggestions

During setup, after the user selects a provider, show a suggested default model name:

| Provider | Suggested Model |
|----------|----------------|
| openai | gpt-4o |
| anthropic | claude-sonnet-4-20250514 |
| gemini | gemini-2.0-flash |
| ollama | llama3 |
| completions | (none — user must specify) |
| responses | (none — user must specify) |
| openrouter | (none — depends on upstream) |
| huggingface | (none — depends on model) |
| mistral | mistral-large-latest |
| qwen | qwen-max |
| kimi | moonshot-v1-8k |
| zai | (none) |
| minimax | (none) |

## Tests

**Unit tests for `setup.rs`:**
- Config file generation with valid user inputs
- Config file generation with all defaults accepted
- Existing config values used as defaults
- Config directory created if it doesn't exist

**CLI tests:**
- `--setup` flag parses correctly

**Integration tests:**
- First-run detection triggers setup in interactive mode
- Non-interactive first-run prints hint message
- `--setup` on non-interactive terminal exits with error
- `--quiet` suppresses first-run message

## Acceptance Criteria

- [ ] `cmdify --setup` launches interactive wizard on a terminal
- [ ] `cmdify --setup` exits with error on a non-interactive terminal (piped)
- [ ] First run on an interactive terminal auto-enters setup mode
- [ ] First run on a non-interactive terminal prints hint to stderr
- [ ] `cmdify -q "list files"` on first run prints no hint (quiet mode)
- [ ] Setup writes valid TOML to `~/.config/cmdify/config.toml`
- [ ] Setup does NOT write API keys to the config file
- [ ] Setup prints shell export commands for API keys
- [ ] Re-running setup with existing config uses current values as defaults
- [ ] `make check` passes
