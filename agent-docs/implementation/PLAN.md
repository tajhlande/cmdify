# cmdify — Phased Implementation Plan

## Overview

This plan breaks the full `cmdify` design into incremental, testable phases. Each phase produces a working binary and a passing test suite. Phases are ordered so that each builds on the last and no phase requires a rewrite of prior work.

**Versioning:** Bump the minor version (`0.x.0` → `0.(x+1).0`) when beginning implementation of a new phase. When all phases are complete, bump to `1.0.0`.

---

## Phase Summary

| Phase | Status | Title | Scope | Key Deliverables |
|-------|--------|-------|-------|-----------------|
| 1 | ✅ | Minimal MVP | `/completions` provider, no tools | Working binary, basic UX, env config |
| 2 | ✅ | `find_command` Tool | Add command discovery tool | Tool trait, registry, tool call loop |
| 3 | ⬜ | `ask_user` Tool | Add interactive clarification tool | Interactive stdin/stderr UX |
| 4 | ⬜ | OpenRouter & HuggingFace | Two more OpenAI-compat providers | Named provider pattern, shared completions impl |
| 5 | ⬜ | Gemini, OpenAI, Anthropic | First-class providers, distinct wire formats | Three new providers, AuthStyle::QueryParam |
| 6 | ⬜ | Responses & Remaining | Responses API + Z.ai, Minimax, Qwen, Kimi, Mistral, Ollama | Full provider coverage |
| 7 | ⬜ | Cross-Compilation | Build targets for all platforms | Makefile dist, Raspbian arm, Apple Intel/Silicon |
| 8 | ⬜ | CI/CD & Distribution | GitHub Actions, releases, polish | Automated testing, release workflow, docs |
| 9 | 📝 | Safety Check | Unsafe command detection, `--unsafe` flag | Safety module, pattern matching, CLI flag |
| 10 | 📝 | Interactive Setup | `--setup` flag, first-run detection, config wizard | Setup module, interactive prompts, config file creation |
| 11 | 📝 | Debug Mode | Debug logging, `--debug` flag | Debug logging module, stderr trace output, configurable verbosity |

---

## Phase Dependencies

```
Phase 1 ──→ Phase 2 ──→ Phase 3 ──→ Phase 4 ──→ Phase 5 ──→ Phase 6
                                                                │
                                                                ▼
Phase 7 (can start after Phase 1, but benefits from Phase 5)
                                                                 │
                                                                 ▼
Phase 8 (requires Phase 7 complete)
                                                                 │
                                                                 ▼
Phase 9 (can start after Phase 1, independent of other phases)
                                                                 │
                                                                 ▼
Phase 10 (requires Phase 1, benefits from Phase 6 for full provider list)
```

Phase 7 (cross-compilation) can begin any time after Phase 1 produces a working binary, since the `Makefile dist` target is independent of provider/tool complexity. However, it benefits from Phase 5+ since those phases finalize the dependency list.

Phase 11 (debug mode) is independent and can start after Phase 1. It enhances observability across all other phases once in place.

---

## Cross-Phase Notes

### Command Execution Logging (added during Phase 2)

Every subprocess spawned by cmdify is logged to a file for auditing. This covers:
- `find_command` tool lookups (`command -v` and `which` fallback)
- `--yolo` command executions

**Log file location** (XDG Base Directory compliant):
- `$XDG_STATE_HOME/cmdify/history.log` if `XDG_STATE_HOME` is set
- `$HOME/.local/state/cmdify/history.log` otherwise

**Log format:**
```
[2026-04-03T16:30:00Z] [output] [completions/llama3] ls -la
[2026-04-03T16:30:00Z] [find_command] [completions/llama3] command -v ls
```

**Implementation details:**
- `src/logger.rs` — `CmdifyLogger` struct, best-effort file open (silently degrades if unavailable)
- `Tool` trait's `execute()` accepts `Option<&CmdifyLogger>` to pass logging context through
- Logger is created in `main.rs` with model/provider info, passed to orchestrator and tool registry
- No env var or CLI flag to configure — logging is always-on for subprocess execution
- Uses `chrono` for ISO 8601 UTC timestamps

---

## Design Documents

- [DESIGN.md](../design/DESIGN.md) — Architecture & module structure
- [PROVIDERS.md](../design/PROVIDERS.md) — Provider trait, wire formats, factory
- [TOOLS.md](../design/TOOLS.md) — Tool trait, registry, definitions, loop
- [BUILD.md](../design/BUILD.md) — Configuration, dependencies, testing
- [CRITIQUES.md](../design/CRITIQUES.md) — Resolved design issues

---

## Phase Documents

- [Phase 1 — Minimal MVP](./phase-1-mvp.md)
- [Phase 2 — find_command Tool](./phase-2-find-command.md)
- [Phase 3 — ask_user Tool](./phase-3-ask-user.md)
- [Phase 4 — OpenRouter & HuggingFace](./phase-4-openrouter-huggingface.md)
- [Phase 5 — Gemini, OpenAI, Anthropic](./phase-5-gemini-openai-anthropic.md)
- [Phase 6 — Responses & Remaining Providers](./phase-6-responses-remaining.md)
- [Phase 7 — Cross-Compilation](./phase-7-cross-compilation.md)
- [Phase 8 — CI/CD & Distribution](./phase-8-ci-distribution.md)
- [Phase 9 — Safety Check](./phase-9-safety-check.md)
- [Phase 10 — Interactive Setup](./phase-10-interactive-setup.md)
- [Phase 11 — Debug Mode](./phase-11-debug-mode.md)
- [Test Strategy](./test-strategy.md)
- [Live Testing](./live-testing.md)
