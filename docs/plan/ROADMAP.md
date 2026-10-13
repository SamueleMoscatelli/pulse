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
- [ ] P2.3 AI-SEC-002 prompt-injection phrases
- [ ] P2.4 AI-EFF-001 & AI-EFF-002 token budgets
- [ ] P2.5 AI-COG-001 contradictory directives
- [ ] P2.6 Suppressions (inline + config)
- [ ] P2.7 Scoring, ratings & quality gate
- [ ] P2.8 Terminal reporter, `scan` & `ci` commands
- [ ] P2.9 End-to-end tests, benchmarks, v0.1.0

## Phase 3 — Security pack
- [ ] P3.1 AI-SEC-003 hardcoded secrets
- [ ] P3.2 AI-SEC-004/005/006 MCP config security
- [ ] P3.3 AI-SEC-007 over-permissive permissions
- [ ] P3.4 AI-SEC-008/009 hook command safety
- [ ] P3.5 AI-SEC-010/011 exfiltration & hidden instructions
- [ ] P3.6 AI-SEC-012/013 tool poisoning & sensitive paths

## Phase 4 — Structure & validity pack
- [ ] P4.1 AI-STR-001 & AI-STR-004 frontmatter schemas
- [ ] P4.2 AI-STR-006 & AI-STR-005 hooks & tool names
- [ ] P4.3 AI-STR-003 references & import graph
- [ ] P4.4 AI-STR-007 duplicate asset names

## Phase 5 — Efficiency & cognitive pack
- [ ] P5.1 AI-EFF-003 duplicated content (SimHash)
- [ ] P5.2 AI-EFF-004/005/006 bulk, sprawl, oversized
- [ ] P5.3 AI-EFF-007, AI-COG-002, AI-COG-004 lexicon rules
- [ ] P5.4 AI-COG-003/006/007 negative, routing, structure
- [ ] P5.5 AI-COG-005 cross-file contradictions

## Phase 6 — Reports & workflow (v0.2.0)
- [ ] P6.1 JSON & SARIF reporters
- [ ] P6.2 Markdown & GitHub reporters
- [ ] P6.3 Baseline, `--new-only`, `--changed-since`
- [ ] P6.4 `pulse rules` & `pulse explain`
- [ ] P6.5 Declarative custom rules

## Phase 7 — Hardening & release (v1.0.0)
- [ ] P7.1 Performance hardening & budget enforcement
- [ ] P7.2 Release pipeline, GitHub Action, pre-commit
- [ ] P7.3 Docs, license, dogfooding in CI

## Backlog (unscheduled; promote by adding a card + roadmap line)
`pulse fix` autofixes · LSP server & VS Code extension · WASM playground · opt-in `--deep` LLM analysis · result cache keyed by content hash · optional tiktoken feature · multilingual injection lexicon · `apm` integration · scanning prompts embedded in source code. · default excludes (`build`, `out`, …) also prune same-named dirs inside asset trees (e.g. `.claude/commands/build/`): consider pruning them only outside hidden dirs. · AI-SEC-001: allow emoji tag sequences (U+1F3F4 + tags + U+E007F, e.g. the England flag) instead of reporting them as tag smuggling.
