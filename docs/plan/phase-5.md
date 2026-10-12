# Phase 5 — Efficiency & cognitive pack

Goal: the context-debt and clarity rules. They are heuristic, so precision comes first: when in doubt, don't fire. Every task follows the `add-rule` skill.

## P5.1 — AI-EFF-003 duplicated content (SimHash)
- **Size** L · **Depends** P4.4
- **Read** `scripts/spec.sh "AI-EFF-003"`; skill `add-rule`
- **Do** `pulse-core::text::simhash` (5-word shingles over normalized text, xxh3, 64-bit SimHash). Project rule: bucket by 4×16-bit bands (LSH) to avoid O(n²), then verify Hamming ≤ 3. Exact duplicate line detection across always-on assets.
- **Done when** check is green; fixtures: the same paragraph in CLAUDE.md and AGENTS.md (fail), a lightly reworded paragraph (fail), two different paragraphs on the same topic (pass); a bench shows 1,000 assets × 20 paragraphs in < 10 ms.

## P5.2 — AI-EFF-004/005/006 bulk, sprawl, oversized
- **Size** M · **Depends** P5.1
- **Read** `scripts/spec.sh "AI-EFF-004"`, `scripts/spec.sh "AI-EFF-005"`, `scripts/spec.sh "AI-EFF-006"`; skill `add-rule`
- **Do** Three straightforward rules; MCP duplicate-server detection normalizes command+args.
- **Done when** check is green; boundary fixtures for each threshold.

## P5.3 — AI-EFF-007, AI-COG-002, AI-COG-004 lexicon rules
- **Size** M · **Depends** P5.2
- **Read** `scripts/spec.sh "AI-EFF-007"`, `scripts/spec.sh "AI-COG-002"`, `scripts/spec.sh "AI-COG-004"`; skill `add-rule`
- **Do** `lexicon/style.rs` (FILLER with replacements, VAGUE, ACRONYMS); the shared aggregation helper "≥ N hits → one summary finding, else Info per hit" lives in `pulse-rules/src/util.rs`.
- **Done when** check is green; fixtures with realistic CLAUDE.md prose (pass) vs bloated prose (fail); `token_waste` asserted for EFF-007.

## P5.4 — AI-COG-003/006/007 negative, routing, structure
- **Size** M · **Depends** P5.3
- **Read** `scripts/spec.sh "AI-COG-003"`, `scripts/spec.sh "AI-COG-006"`, `scripts/spec.sh "AI-COG-007"`; skill `add-rule`
- **Do** Reuse `cognitive::directives`.
- **Done when** check is green; fixtures per rule, including "Don't use X; use Y instead" (pass for COG-003) and "Use when the user asks to …" descriptions (pass for COG-006).

## P5.5 — AI-COG-005 cross-file contradictions
- **Size** M · **Depends** P5.4
- **Read** `scripts/spec.sh "AI-COG-005"`, `scripts/spec.sh "AI-COG-001"`; skill `add-rule`
- **Do** Project rule; build load-together groups (root Always set + imports; each nested CLAUDE.md/AGENTS.md with its ancestors); reuse the directive extraction and the AI-COG-001 comparison logic (refactor it into a shared `fn conflicts(a, b) -> Option<Conflict>` if not already).
- **Done when** check is green; fixtures: root says "use pnpm", `packages/x/CLAUDE.md` says "use npm" (fail, reported on the nested file); unrelated nested dirs (pass).
