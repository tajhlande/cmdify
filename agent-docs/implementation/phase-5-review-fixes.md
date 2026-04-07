# Phase 5 — Review Fix Plan

Addresses the findings in `agent-docs/review/phase 5 review and recommendations.md`.

**Status: COMPLETE** — all fixes applied, 277 tests passing, clippy/fmt clean.

---

## Fix 1 — `debug!` macro double-format bug (medium severity) ✅

**File:** `src/debug.rs`

**Problem:** The `debug!` and `debug_at!` macros called `format_line()`, then passed the result to `emit_line()` / `emit_line_at()`, which called `format_line()` again. Every debug line was emitted as:
```
DEBUG +42ms | DEBUG +42ms | the actual message
```

**Fix:** Changed macros to call `eprintln!` directly with the already-formatted string. Removed `emit_line` and `emit_line_at` (only used by the broken macros). Added `debug_macro_single_prefix` test.

---

## Fix 2 — `_file_sources` discarded in main.rs ✅

**File:** `src/main.rs`

**Problem:** `Config::from_env()` sources were bound to `_file_sources` and thrown away. Only CLI-override sources got logged under `-d`.

**Fix:** Renamed to `env_sources` / `cli_sources`, loop over both in the debug output section.

---

## Fix 3 — Refactor `check_rm_targets` + `check_targets` duplication ✅

**File:** `src/safety.rs`

**Problem:** Two near-identical functions with inline copies of path arrays.

**Fix:** Extracted module-level `BROAD_PATHS`, `SENSITIVE_PREFIXES`, `HOME_PATTERNS` consts. Created shared `check_path_targets(tokens: &[String])` helper. Both callers delegate to it.

---

## Fix 4 — Unify `is_flagged_command` with `check_command` ✅

**File:** `src/safety.rs`

**Problem:** `is_flagged_command()` maintained a separate `matches!()` list duplicating `check_command()`.

**Fix:** Replaced with `TARGET_CHECKED_COMMANDS` const array. `is_flagged_command` now checks membership against this single source of truth. `check_command` still uses its match block (it needs different categories per command), but the lists are derived from the same conceptual set.

---

## Fix 5 — Fix `tool_level_cli_overrides_env` test ✅

**File:** `src/config.rs`, `src/app.rs`

**Problem:** Test used `Some(3).unwrap_or(config.tool_level)` — testing `Option::unwrap_or`, not our code.

**Fix:** Removed from `config.rs`. Added `tool_level_cli_overrides_env_level` to `app.rs` that tests through `apply_cli_overrides`.

---

## Fix 6 — Extract `Spinner::shutdown()` private method ✅

**File:** `src/spinner.rs`

**Problem:** `stop()` and `drop()` had identical cleanup code.

**Fix:** Extracted `fn shutdown(&mut self)`. Both `stop()` and `drop()` call it.

---

## Fix 7 — Remove stale `#[allow(dead_code)]` on `Config.allow_unsafe` ✅

**File:** `src/config.rs`

**Fix:** Removed annotation. Clippy confirms no warning.

---

## Additional — `check_structural` guard condition ✅

**File:** `src/safety.rs`

**Problem:** `raw.contains('$') && raw.contains('(')` guard was redundant and had false positives.

**Fix:** Removed guard; `raw.find("$(")` is called directly (same O(n), no false positives).
