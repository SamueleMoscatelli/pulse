# Roadmap

Status source of truth for `/next-task`. Mark tasks only with `scripts/task.sh done <ID>`. Each task is one Claude Code session; task cards live in `docs/plan/phase-<N>.md`.

## Phase 0 — Foundations
- [x] P0.1 Cargo workspace & tooling scaffold
- [x] P0.2 Core model types, Asset & LineIndex
- [x] P0.3 Config (`pulse.toml`) & `pulse init`

## Phase 1 — Discovery & parsing
- [x] P1.1 Asset discovery (walker + classification)
- [x] P1.2 Text utilities: token estimator & normalization
- [x] P1.3 Markdown view (frontmatter, structure, links, imports)
- [x] P1.4 JSON views (MCP, settings/hooks, tools)

## Phase 2 — Engine & MVP (v0.1.0)
- [x] P2.1 Engine pipeline, rule registry, fixture harness, AI-STR-002
- [x] P2.2 AI-SEC-001 invisible/bidi Unicode
- [x] P2.3 AI-SEC-002 prompt-injection phrases
- [x] P2.4 AI-EFF-001 & AI-EFF-002 token budgets
- [x] P2.5 AI-COG-001 contradictory directives
- [x] P2.6 Suppressions (inline + config)
- [x] P2.7 Scoring, ratings & quality gate
- [x] P2.8 Terminal reporter, `scan` & `ci` commands
- [x] P2.9 End-to-end tests, benchmarks, v0.1.0

## Phase 3 — Security pack
- [x] P3.1 AI-SEC-003 hardcoded secrets
- [x] P3.2 AI-SEC-004/005/006 MCP config security
- [x] P3.3 AI-SEC-007 over-permissive permissions
- [x] P3.4 AI-SEC-008/009 hook command safety
- [x] P3.5 AI-SEC-010/011 exfiltration & hidden instructions
- [x] P3.6 AI-SEC-012/013 tool poisoning & sensitive paths

## Phase 4 — Structure & validity pack
- [x] P4.1 AI-STR-001 & AI-STR-004 frontmatter schemas
- [x] P4.2 AI-STR-006 & AI-STR-005 hooks & tool names
- [x] P4.3 AI-STR-003 references & import graph
- [x] P4.4 AI-STR-007 duplicate asset names

## Phase 5 — Efficiency & cognitive pack
- [x] P5.1 AI-EFF-003 duplicated content (SimHash)
- [x] P5.2 AI-EFF-004/005/006 bulk, sprawl, oversized
- [x] P5.3 AI-EFF-007, AI-COG-002, AI-COG-004 lexicon rules
- [x] P5.4 AI-COG-003/006/007 negative, routing, structure
- [x] P5.5 AI-COG-005 cross-file contradictions

## Phase 6 — Reports & workflow (v0.2.0)
- [x] P6.1 JSON & SARIF reporters
- [x] P6.2 Markdown & GitHub reporters
- [x] P6.3 Baseline, `--new-only`, `--changed-since`
- [x] P6.4 `pulse rules` & `pulse explain`
- [x] P6.5 Declarative custom rules

## Phase 7 — Hardening & release (v1.0.0)
- [x] P7.1 Performance hardening & budget enforcement
- [x] P7.2 Release pipeline, GitHub Action, pre-commit
- [x] P7.3 Docs, license, dogfooding in CI

## Phase 8 — Real-world accuracy (v1.1.0)
- [x] P8.1 Before/after comparison harness
- [x] P8.2 Recognize library assets outside activation folders
- [x] P8.3 AI-STR-008: asset in a place no assistant loads
- [x] P8.4 AI-COG-003 redesign: real prohibitions only
- [x] P8.5 Visible advice: fixes by default, `-v` for all
- [x] P8.6 Context load front and center
- [x] P8.7 False-positive guards from the corpus
- [x] P8.8 Evaluation report and release candidate

## Phase 9 — Kiro support (v1.2.0)
- [x] P9.1 Kiro corpus baseline
- [x] P9.2 Kiro as an assistant: discovery and load modes
- [ ] P9.3 Steering frontmatter and file references
- [ ] P9.4 Kiro MCP config
- [ ] P9.5 Kiro hooks, legacy and current
- [ ] P9.6 Kiro custom agents, JSON and Markdown
- [ ] P9.7 Kiro powers as libraries
- [ ] P9.8 Evaluation report and release

## Post-release follow-ups
Homebrew tap, Marketplace listing, usage tracking and the release routine: [post-release.md](post-release.md).

## Backlog (unscheduled; promote by adding a card + roadmap line)
`pulse fix` autofixes · LSP server & VS Code extension · WASM playground · opt-in `--deep` LLM analysis · result cache keyed by content hash · optional tiktoken feature · multilingual injection lexicon · `apm` integration · scanning prompts embedded in source code. · default excludes (`build`, `out`, …) also prune same-named dirs inside asset trees (e.g. `.claude/commands/build/`): consider pruning them only outside hidden dirs. · AI-SEC-001: allow emoji tag sequences (U+1F3F4 + tags + U+E007F, e.g. the England flag) instead of reporting them as tag smuggling. · AI-EFF-002: the top-5 contributor list can push the message past 100 chars; consider basenames or a shorter format once reporters show details. · `pulse explain` for custom (`X-`) rules: print name, message and remediation from `pulse.toml`. · AI-SEC-007: map Copilot `.prompt.md` tool names (`runCommands`, `runInTerminal`, `fetch`) to Bash/network. · AI-COG-005: index directives across files like AI-COG-001 (still pairwise). · Pin `BENCH_REF_MS` in `scripts/bench.sh` (provisional 24 ms, ADR-038) with one `scripts/bench.sh` run on an Apple M-series laptop. · Discovery: add `.devin/rules/*.md` (Windsurf) and `.cline/rules/` (Cline) load folders (ADR-042). · AI-STR-008: honor VS Code `chat.*Locations` folders from `.vscode/settings.json`.  · Context load: count `@import`ed files per assistant in `metrics.always_on_tokens` and the CONTEXT LOAD block, as AI-EFF-002 does (ADR-045). · Discovery: classify every `.github/hooks/*.json` as Copilot Hooks, not only `hooks.json` (ADR-046). · AI-STR-001: on Copilot agents, report the legacy chat-mode key `mode` with "not used by agents" instead of the Levenshtein hint "did you mean model". · `pulse.toml`: v1.0.0 rejects keys added later (`library`) with a hard error; consider warning on unknown top-level keys for forward compatibility. · AI-EFF-003 (and other cross-file always-on checks): pair files only when some assistant loads both (`loaded_by`); on ECC `.kiro/steering/git-workflow.md` is reported as paid twice with `.github/copilot-instructions.md`, which no assistant loads together (P9.2). · Kiro: `.kiro/specs/*/{requirements,design,tasks}.md` (loaded in spec sessions only). · Kiro: global `~/.kiro/` steering, hooks and agents (user-level, like other assistants' user files).
