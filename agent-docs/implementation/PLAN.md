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
| 3 | ✅ | `ask_user` Tool | Add interactive clarification tool | Interactive stdin/stderr UX |
| 4 | ✅ | Tool Levels | Numbered tool level system, `--list-tools` | Progressive tool disclosure, config |
| 5 | ✅ | Safety Check | Modular prompt, LLM guidance, semantic checks | Three-layer safety, system prompt split, shlex tokenization |
| 6 | ✅ | OpenRouter & HuggingFace | Two more OpenAI-compat providers | Named provider pattern, shared completions impl |
| 7 | ✅ | Gemini, OpenAI, Anthropic | First-class providers, distinct wire formats | Three new providers, AuthStyle::QueryParam |
| 8 | ✅ | Responses & Remaining | Responses API + Z.ai, Minimax, Qwen, Kimi, Mistral, Ollama | Full provider coverage |
| 9 | ✅ | Cross-Compilation | Build targets for all platforms | Makefile dist, cross for Linux, Apple Intel/Silicon |
| 10 | ✅ | CI/CD & Distribution | GitHub Actions, releases, polish | Automated testing, release workflow, docs |
| 11 | 📝 | Interactive Setup | `--setup` flag, first-run detection, config wizard | Setup module, interactive prompts, config file creation |
| 12 | ✅ | Debug Mode | Debug logging, `--debug` flag | Debug logging module, stderr trace output, configurable verbosity |

---

## Phase Dependencies

```
Phase 1 ──→ Phase 2 ──→ Phase 3 ──→ Phase 4 ──→ Phase 5 ──→ Phase 6 ──→ Phase 7 ──→ Phase 8
                                                                  │
                                                                  ▼
Phase 9 (can start after Phase 1, but benefits from Phase 7)
                                                                  │
                                                                  ▼
Phase 10 (can start after Phase 1, independent of other phases)
                                                                  │
                                                                  ▼
Phase 11 (requires Phase 1, independent of other phases)
                                                                  │
                                                                  ▼
Phase 12 (can start after Phase 1, independent of other phases)
```

Phase 4 (tool levels) follows Phase 3 since it restructures the tool system that Phases 2 and 3 built. Provider phases (6–8) can proceed without it since the tool level infrastructure is independent of provider logic.

Phase 5 (safety check) follows Phase 4 since the modular prompt system introduced there will include safety guidance. It is independent of provider work and provides a safety foundation before provider phases add more capabilities.

Phase 9 (cross-compilation) can begin any time after Phase 1 produces a working binary, since the `Makefile dist` target is independent of provider/tool complexity. However, it benefits from Phase 7+ since those phases finalize the dependency list.

Phase 12 (debug mode) is independent and can start after Phase 1. It enhances observability across all other phases once in place.

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

### Tool Levels (added during Phase 4)

Tools are organized into numbered levels (0–3) providing progressive environment awareness. See [Phase 4 — Tool Levels](./phase-4-tool-levels.md) for full details.

| Level | Name | Tools | Risk Profile |
|-------|------|-------|-------------|
| 0 | none | *(none)* | No tool access |
| 1 | core | `ask_user`, `find_command`, `list_current_directory` | Interactive clarification, command checks, cwd listing |
| 2 | local | `command_help`, `list_any_directory`, `pwd` | Read-only filesystem access |
| 3 | system | `get_env`, `list_processes` | System introspection |

---

## Design Documents

- [DESIGN.md](../design/DESIGN.md) — Architecture & module structure
- [PROVIDERS.md](../design/PROVIDERS.md) — Provider trait, wire formats, factory
- [TOOLS.md](../design/TOOLS.md) — Tool trait, registry, definitions, levels, loop
- [BUILD.md](../design/BUILD.md) — Configuration, dependencies, testing
- [CRITIQUES.md](../design/CRITIQUES.md) — Resolved design issues

---

## Phase Documents

- [Phase 1 — Minimal MVP](./phase-1-mvp.md)
- [Phase 2 — find_command Tool](./phase-2-find-command.md)
- [Phase 3 — ask_user Tool](./phase-3-ask-user.md)
- [Phase 4 — Tool Levels](./phase-4-tool-levels.md)
- [Phase 5 — Safety Check](./phase-5-safety-check.md)
- [Phase 6 — OpenRouter & HuggingFace](./phase-6-openrouter-huggingface.md)
- [Phase 7 — Gemini, OpenAI, Anthropic](./phase-7-gemini-openai-anthropic.md)
- [Phase 8 — Responses & Remaining Providers](./phase-8-responses-remaining.md)
- [Phase 9 — Cross-Compilation](./phase-9-cross-compilation.md)
- [Phase 10 — CI/CD & Distribution](./phase-10-ci-distribution.md)
- [Phase 11 — Interactive Setup](./phase-11-interactive-setup.md)
- [Phase 12 — Debug Mode](./phase-12-debug-mode.md)
- [Test Strategy](./test-strategy.md)
- [Live Testing](./live-testing.md)
