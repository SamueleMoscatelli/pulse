# Phase 7 — Hardening & release (v1.0.0)

Goal: provably fast, robust against hostile input, easy to install, and dogfooded.

## P7.1 — Performance hardening & budget enforcement
- **Size** M · **Depends** P6.5
- **Read** `scripts/spec.sh "Performance budget"`, `scripts/spec.sh "Performance design"`, `docs/perf.md`
- **Do** Profile `scan` on the 1,000-asset generated tree (`cargo build --profile profiling` with debug symbols + `samply` or `cargo flamegraph` if installed, else per-phase `--timings`). Fix the top 3 hotspots. Make `scripts/bench.sh` fail on budget regressions and add it to CI as a non-blocking job on macOS. Add a fuzz-ish robustness test: random bytes / huge lines / deeply nested JSON / 10k HTML comments must not panic or exceed 2× the time budget.
  - (from P2.9, ADR-019) v0.1.0 numbers in `docs/perf.md` come from a 2019 Intel i5 and miss the budget (startup 6.1 ms, typical 46 ms, large 317 ms); re-measure on Apple M-series or a `macos-14` runner first. Discovery is ~80% of scan time.
  - (from P5.1, ADR-029) re-measure the `duplicated_content` bench (target < 10 ms; 10.9 ms on the i5) on M-series. AI-EFF-003 findings cost ~9 µs each downstream in the engine (1,600 on `large`).
- **Done when** budgets met (update `docs/perf.md`); the robustness test is green.

## P7.2 — Release pipeline, GitHub Action, pre-commit
- **Size** M · **Depends** P7.1
- **Read** `scripts/spec.sh "Commands"` (ci), `scripts/spec.sh "GitHub ("`
- **Do** `cargo-dist` (or a hand-written release workflow if cargo-dist is unavailable) for macOS arm64/x64, Linux x64/arm64 (musl), Windows x64; shell + PowerShell installers; Homebrew tap formula template. `action.yml` (composite: download the release binary, run `pulse ci`, upload SARIF optionally). `.pre-commit-hooks.yaml`. Ask the user before creating any remote resources or pushing tags.
- **Done when** workflows lint (`actionlint` if available); local `dist plan`, or a dry run of the workflow logic, succeeds.

## P7.3 — Docs, license, dogfooding in CI
- **Size** M · **Depends** P7.2
- **Read** `scripts/spec.sh` (no args: TOC), `scripts/spec.sh "Vision"`
- **Do** Full README (pitch, install, quickstart, sample output, rule table generated from the registry by `pulse rules --format markdown` or a test that keeps the README table in sync), `LICENSE` (Apache-2.0 full text), `CONTRIBUTING.md` (pointing at the add-rule skill flow for humans too), a CI job running `pulse ci` on this repo's own AI assets.
- **Done when** check is green; a README sync test passes; the dogfood job passes.
