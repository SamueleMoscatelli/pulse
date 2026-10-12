---
paths:
  - "**/*.rs"
---

# Rust conventions for Pulse

- Edition 2024, stable. `unsafe` is forbidden.
- Statics: `std::sync::LazyLock` for compiled regexes and automatons. `expect("static pattern")` is allowed only there.
- Slicing strings: only on byte offsets that came from the same string's matches or char boundaries; when in doubt use `str::get(..)`.
- Offsets are `u32` in `Span` (assets are capped well below 4 GiB); convert with `u32::try_from` and turn any failure into an AI-STR-002 finding.
- Collections that reach output: `Vec` sorted, or `BTreeMap`. Never iterate a `HashMap` into a report.
- Prefer `&str`/`Cow` over `String` in hot paths; `SmallVec` only after a bench shows it matters.
- Public items get a one-line `///` doc; no essay comments. Comment the *why* of non-obvious heuristics with the spec ID (`// AI-COG-001 FP guard: scoped qualifier`).
- Tests: `#[test] fn <behavior>_<condition>()` names; use `insta::assert_snapshot!` for multi-line output; no sleeping or network.
