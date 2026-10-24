# Contributing to Pulse

Thanks for helping. Pulse is a Rust workspace that builds one binary, `pulse`. It has three
hard constraints: it is **deterministic** (same input, same output, findings sorted by path,
line, column and rule), **offline** (no network, ever), and **fast** (a typical repository
scans in under 20 ms).

## Setup

You need the stable Rust toolchain (`rustup` picks it up from `rust-toolchain.toml`) and, for snapshot review,
[`cargo-insta`](https://insta.rs/).

```sh
scripts/check.sh               # fmt + clippy + tests; must be green before a PR
scripts/check.sh -p pulse-rules  # faster inner loop for a single crate
cargo run -q -p pulse-cli -- scan -C <dir>
cargo insta test -p <crate>    # then review every .snap.new before accepting it
```

## Where things live

| Path | What |
|---|---|
| `crates/pulse-core` | asset model, config, discovery, parsing, text utilities |
| `crates/pulse-rules` | the rules, grouped by category, plus shared word lists in `lexicon/` |
| `crates/pulse-engine` | pipeline, suppressions, scoring, Quality Gate, baseline |
| `crates/pulse-report` | terminal, JSON, SARIF, Markdown and GitHub reports |
| `crates/pulse-cli` | the `pulse` binary |
| `docs/spec/` | the specification (`scripts/spec.sh` with no arguments prints its table of contents) |
| `docs/rules/<ID>.md` | one page per rule, embedded in the binary |
| `docs/decisions.md` | architecture decision records (ADRs) |

## Adding or changing a rule

Follow the checklist in [`.claude/skills/add-rule/SKILL.md`](.claude/skills/add-rule/SKILL.md).
Coding agents use the same file as a skill. In short:

1. Specify the rule in `docs/spec/03-rules.md`: detection, severity and false-positive guards.
2. Implement it in `crates/pulse-rules/src/<category>/<topic>.rs` from
   [`rule-template.rs`](.claude/skills/add-rule/rule-template.rs), and register it in
   `crates/pulse-rules/src/lib.rs`.
3. Add fixtures in `crates/pulse-rules/tests/fixtures/<ID>/`: at least one `fail_*` and two
   `pass_*` files, the passes being realistic near-misses.
4. Write `docs/rules/<ID>.md` from [`doc-template.md`](.claude/skills/add-rule/doc-template.md)
   and add the ID to `crates/pulse-rules/src/docs.rs`.
5. Regenerate the README rule table: `UPDATE_README=1 cargo test -p pulse-cli readme`.

The fixture harness fails when a registered rule has no fixtures or docs, or when a doc does not
follow the template.

## Code conventions

- No `unwrap`/`expect` outside tests. Libraries use `thiserror`, the CLI uses `anyhow`. A
  malformed asset becomes a finding; Pulse never panics or aborts on one.
- On the hot path, compile regexes once (`LazyLock`), use only the `regex` crate (no
  backtracking engines), prefer `memchr`/`aho-corasick`, and avoid allocating on input that
  does not match.
- Add dependencies only from the table in `docs/spec/01-architecture.md` ("Dependencies"), or
  record an ADR that justifies the new one.
- Unit tests go next to the code; rule behavior is tested through fixtures. Build invisible
  Unicode and secret-like test content in code, so it never lands in a committed file.

## Changing the spec

When the implementation has to deviate from the spec, update the relevant section in
`docs/spec/` and append an ADR to `docs/decisions.md`. To revisit an existing decision, add a
new ADR that supersedes it rather than editing the old one.

## Pull requests

Keep each PR to one focused change, with a description of what changed and how you verified it.
CI runs `scripts/check.sh` on Linux, macOS and Windows, and dogfoods `pulse ci` on this
repository's own AI assets, so all of these must pass.

By contributing, you agree that your contributions are licensed under the
[Apache License, Version 2.0](LICENSE).
