---
name: add-rule
description: Checklist and templates for implementing or changing a Pulse analysis rule (AI-SEC/STR/EFF/COG-NNN or custom). Use whenever a task adds a rule, changes rule detection logic, or adds rule fixtures/docs.
---

# Adding a Pulse rule

The rule's spec is in `scripts/spec.sh "<RULE-ID>"`. Implement exactly that: detection, severities, FP guards.

## Files to touch (and only these)
1. `crates/pulse-rules/src/<category>/<topic>.rs`: the rule (template: [rule-template.rs](rule-template.rs)). Group related rules in one topic file (e.g. `security/mcp.rs`). Add `mod` lines in `<category>/mod.rs`.
2. `crates/pulse-rules/src/lib.rs`: register it in `RULES` (asset scope) or `PROJECT_RULES` (project scope), keeping ID order.
3. Shared word lists go in `crates/pulse-rules/src/lexicon/<name>.rs` as `pub const` slices, plus a `LazyLock` automaton if needed. Never inline big lists in rule files.
4. `crates/pulse-rules/tests/fixtures/<RULE-ID>/`: ≥ 1 `fail_*` and ≥ 2 `pass_*` (realistic near-misses). Use `.expect` files (`line:col` per line) when location matters. Use `_kind` when the kind can't be inferred.
5. `docs/rules/<RULE-ID>.md` from [doc-template.md](doc-template.md), and add the ID to the `rule_docs!` list in `crates/pulse-rules/src/docs.rs` (embedded for SARIF help and `explain`).
6. Invisible characters, secrets and other content that would trip editors or push protection: build it in a test in `crates/pulse-rules/tests/fixtures.rs` (string concatenation / `\u{...}` escapes), not in fixture files.

## Implementation rules
- `RuleMeta` is a `static`; `kinds` restricts which assets reach `check`, so don't re-check the kind inside.
- Use the asset's cached views (`asset.markdown()`, `normalized()`, `mcp()`, …). Never re-parse.
- Matching on normalized text: map back with `Normalized::source_span` before building a `Finding`.
- Build messages only when a finding fires. Keep them ≤ 100 chars, starting with the problem, quoting the offending snippet (truncated to 40 chars).
- Read options through `ctx.options::<T>(META.id)` (typed, with defaults). Unknown option keys are config errors.
- EFF rules must set `token_waste`.
- Heuristic rule? Add the FP guards from the spec *and* a pass fixture for each guard.
- Never put secrets in messages: redact them.

## Verify
`scripts/check.sh -p pulse-rules` while iterating, then the full `scripts/check.sh`. The fixture harness fails if fixtures or docs are missing for a registered rule, or if a doc strays from the template (title, header line matching the rule meta, sections in order).
