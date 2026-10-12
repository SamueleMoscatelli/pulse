# Decision log (ADR-lite)

Append-only. One entry per decision that changes or clarifies the spec. Format: `## ADR-NNN — Title (YYYY-MM-DD)` + Context / Decision / Consequences, 3–8 lines total.

## ADR-001 — Rust instead of Go (2026-10-12)
Context: the draft proposed Go with a < 30 ms target. Pulse scans untrusted text with many patterns, on every commit and keystroke.
Decision: Rust 2024. It provides the ripgrep-grade crates `ignore`, `globset`, `regex` (linear time, no ReDoS), `aho-corasick` (SIMD multi-pattern) and `memchr`, ~1 ms startup, no GC, and a single static binary.
Consequences: compile times are slower than Go (mitigated by a 5-crate workspace and incremental builds). The npm/pip/brew distribution story is the same as for Go (prebuilt binaries via cargo-dist).

## ADR-002 — Deterministic, offline core (2026-10-12)
Decision: no network and no LLM calls in the core. A semantic `--deep` mode (LLM-assisted contradiction and ambiguity analysis) may come later as an opt-in feature behind a cargo feature flag.
Consequences: heuristic COG rules default to Low/Medium severities and favor precision.

## ADR-003 — Five-crate workspace (2026-10-12)
Decision: `pulse-core`, `pulse-rules`, `pulse-engine`, `pulse-report`, `pulse-cli`. Rules change most often, so they are isolated for incremental compile and focused context. The engine is a library so a future LSP/WASM build can reuse it.

## ADR-004 — Rule IDs `AI-<CAT>-<NNN>` with four categories (2026-10-12)
Decision: keep the draft's `AI-` prefix. Categories SEC/STR/EFF/COG map to Sonar types Vulnerability/Bug/Code smell. STR was added for validity bugs (frontmatter, schemas, broken refs). Custom rules use `X-<ORG>-<NNN>`.

## ADR-005 — Heuristic token estimation (2026-10-12)
Decision: `max(chars/4, words*1.3) * calibration`, with code at 3.2 chars/token. There is no public Claude tokenizer, and bundling BPE vocabularies adds MBs and slows startup. An optional `tiktoken` feature can come later.

## ADR-006 — Critical security findings are not inline-suppressible (2026-10-12)
Context: the scanned assets may themselves be attacker-controlled. A planted `<!-- pulse-ignore AI-SEC-002 -->` would otherwise defeat the scan.
Decision: AI-SEC-001/002/003/008/012 can only be suppressed through `pulse.toml` `[[ignore]]`, which lives in reviewed config.

## ADR-007 — Severity scale and ratings (2026-10-12)
Decision: Critical/High/Medium/Low/Info (the draft's scale, not Sonar's Blocker/Major/Minor). Security/Reliability/Clarity ratings use the worst open severity; Efficiency uses the context-debt ratio (see 04-scoring-gate.md).
