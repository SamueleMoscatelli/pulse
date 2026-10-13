# Phase 6 — Reports & workflow (v0.2.0)

Goal: CI-grade integration: machine-readable output, PR feedback, new-code focus, discoverable rules, user extensibility.

## P6.1 — JSON & SARIF reporters
- **Size** M · **Depends** P5.5
- **Read** `scripts/spec.sh "JSON ("`, `scripts/spec.sh "SARIF"`
- **Do** `json.rs` (serde structs mirroring the schema, plus a `generated_at` timestamp that snapshot tests override), `sarif.rs` (hand-built with `serde_json::json!` or small structs). Rule help text comes from the embedded `docs/rules/*.md`: add `pulse-rules::docs(id) -> &'static str` via a build-time `include_str!` table (a macro generating the match).
- **Done when** check is green; `insta` snapshots for both; the SARIF output validates against the 2.1.0 schema (vendor the schema under `tests/schemas/` and validate in a test with `jsonschema` as a dev-dependency, or document a manual validation step if that crate is too heavy; add a decision note either way).

## P6.2 — Markdown & GitHub reporters
- **Size** S · **Depends** P6.1
- **Read** `scripts/spec.sh "Markdown ("`, `scripts/spec.sh "GitHub ("`
- **Do** Both reporters, escaping per GitHub workflow-command rules (`%`, `\r`, `\n`, `:` and `,` in properties); `GITHUB_STEP_SUMMARY` append; `ci` auto-format.
- **Done when** check is green; snapshots for both; an escaping unit test.

## P6.3 — Baseline, `--new-only`, `--changed-since`
- **Size** M · **Depends** P6.2
- **Read** `scripts/spec.sh "Fingerprints"`, `scripts/spec.sh "Commands"`
- **Do** `baseline.rs` (read/write, version check), the `pulse baseline` command, `--new-only` + `gate.new_code_only` (ratings/gate on new findings; metrics show both), and `--changed-since <ref>` via `std::process::Command` running `git diff --name-only --relative <ref>...HEAD` plus untracked files; a clear error if git is missing.
- **Done when** check is green; tests: fingerprint stability when lines shift, new-only filtering, gate on new code; a tempdir git repo test for `--changed-since`.

## P6.4 — `pulse rules` & `pulse explain`
- **Size** S · **Depends** P6.3
- **Read** `scripts/spec.sh "Commands"`
- **Do** `rules` table (id, severity (effective, after config), enabled, name), JSON variant; `explain` prints the embedded doc (plain, or lightly styled headings on a TTY); unknown ID → exit 2 with a suggestion. Also fill any gaps in `docs/rules/` and make the harness completeness check strict for docs.
- **Done when** check is green; snapshots for both commands.

## P6.5 — Declarative custom rules
- **Size** M · **Depends** P6.4
- **Read** `scripts/spec.sh "Custom rules (declarative)"`, `scripts/spec.sh "pulse.toml"` (`[[custom_rules]]`)
- **Do** `pulse-rules/src/custom.rs`: compile `CustomRule`s at engine start into dynamic rule objects (`Box<dyn Rule>` stored in the engine next to the static registry); matchers `pattern` | `phrases` | `max_tokens`; they appear in `pulse rules`; config errors (bad regex) → exit 2 with the rule ID.
- [from P2.1] Custom-rule IDs in `[rules]` are accepted but their severity/enabled settings are not applied yet (`pulse-engine` `Resolved::new`); wire them here.
- **Done when** check is green; an engine test with two custom rules; a CLI test showing a custom finding in terminal output.
