# Pulse

Static analyzer and Quality Gate for AI assets (instructions, agents, prompts, skills, tools, MCP configs, hooks). Rust workspace, single binary `pulse`. Deterministic, offline, and fast (scan in < 20 ms is a hard requirement).

## Workflow
- Work is organized in roadmap tasks: `/next-task` implements exactly one, on a `task/<ID>-<slug>` branch, and opens a PR (squash-merged by the user once CI is green). Never commit task work to `main`. Status: `scripts/task.sh status`.
- The spec lives in `docs/spec/`. Read sections, not files: `scripts/spec.sh "<heading words>"` (no args = TOC).
- Spec deviations: amend the spec section and append an ADR to `docs/decisions.md`. Never relitigate an existing ADR silently.

## Commands
- Verify (fmt + clippy + tests, compact output): `scripts/check.sh` (`-p <crate>` for the inner loop)
- Run: `cargo run -q -p pulse-cli -- scan -C <dir>`
- Snapshots: `cargo insta test -p <crate>`, then review each `.snap.new` before accepting

## Layout
`crates/pulse-core` (model, config, discovery, parsing, text utils) ← `pulse-rules` (rules by category + `lexicon/`) ← `pulse-engine` (pipeline, suppress, score, gate, baseline) ← `pulse-report` (terminal/json/sarif/md/github) ← `pulse-cli` (bin). Rule docs: `docs/rules/<ID>.md`. Rule fixtures: `crates/pulse-rules/tests/fixtures/<ID>/`.

## Conventions
- Adding or changing a rule: use the `add-rule` skill.
- Hot path: no per-call regex compilation (use `LazyLock`), no allocation on non-matching input, `regex` crate only (never backtracking engines), prefer `memchr`/`aho-corasick`.
- No `unwrap`/`expect` outside tests. Libs use `thiserror`, the CLI uses `anyhow`. A bad asset becomes a finding, never a panic or an abort.
- Output must be deterministic: sort findings by (path, line, col, rule).
- New dependencies: only from the table in `scripts/spec.sh "Dependencies"`, or record an ADR.
- Tests: unit tests next to the code; rule behavior via fixtures. Generate invisible/secret-like test content in code, never commit it raw.

## Token hygiene
- Don't read `target/`, lockfiles or whole spec files. Use `scripts/spec.sh` and targeted reads.
- `.rs` files are auto-formatted by a hook after each edit; don't run `cargo fmt` yourself.
