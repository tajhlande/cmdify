# aicmd — Phased Implementation Plan

## Overview

This plan breaks the full `aicmd` design into incremental, testable phases. Each phase produces a working binary and a passing test suite. Phases are ordered so that each builds on the last and no phase requires a rewrite of prior work.

---

## Phase Summary

| Phase | Title | Scope | Key Deliverables |
|-------|-------|-------|-----------------|
| 1 | Minimal MVP | `/completions` provider, no tools | Working binary, basic UX, env config |
| 2 | `find_command` Tool | Add command discovery tool | Tool trait, registry, tool call loop |
| 3 | `ask_user` Tool | Add interactive clarification tool | Interactive stdin/stderr UX |
| 4 | OpenRouter & HuggingFace | Two more OpenAI-compat providers | Named provider pattern, shared completions impl |
| 5 | Gemini, OpenAI, Anthropic | First-class providers, distinct wire formats | Three new providers, AuthStyle::QueryParam |
| 6 | Responses & Remaining | Responses API + Z.ai, Minimax, Qwen, Kimi, Mistral | Full provider coverage |
| 7 | Cross-Compilation | Build targets for all platforms | Makefile dist, Raspbian arm, Apple Intel/Silicon |
| 8 | CI/CD & Distribution | GitHub Actions, releases, polish | Automated testing, release workflow, docs |

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
```

Phase 7 (cross-compilation) can begin any time after Phase 1 produces a working binary, since the `Makefile dist` target is independent of provider/tool complexity. However, it benefits from Phase 5+ since those phases finalize the dependency list.

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
- [Test Strategy](./test-strategy.md)
- [Live Testing](./live-testing.md)
