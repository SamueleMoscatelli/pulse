# Phase 0 — Foundations

Goal: a compiling workspace with tooling, the core vocabulary types, and configuration. No analysis yet.

## P0.1 — Cargo workspace & tooling scaffold
- **Size** M · **Depends** —
- **Read** `scripts/spec.sh "Workspace layout"`, `scripts/spec.sh "Error handling"`, `scripts/spec.sh "Performance design"` (release profile only)
- **Do**
  - Root `Cargo.toml`: `[workspace]` with `resolver = "3"`, members `crates/*`; `[workspace.package]` (version 0.1.0, edition 2024, license "Apache-2.0", repository `https://github.com/SamueleMoscatelli/pulse`, rust-version = current stable); `[workspace.lints]` (`rust.unsafe_code = "forbid"`, clippy `unwrap_used`, `expect_used`, `dbg_macro`, `todo`, `print_stdout` = "warn"; allow `print_stdout` in `pulse-cli` only); `[profile.release]` per spec; `[profile.dev] debug = "line-tables-only"` for faster builds.
  - Five crates (`pulse-core`, `pulse-rules`, `pulse-engine`, `pulse-report` as libs; `pulse-cli` as bin named `pulse`), each with `lints.workspace = true` and the dependency edges from the spec.
  - `pulse-cli`: `clap` derive with subcommands `init`, `scan`, `ci`, `baseline`, `rules`, `explain` as stubs (print "not implemented yet" to stderr, exit 2); `--version` works.
  - `rust-toolchain.toml` (stable + rustfmt, clippy), `rustfmt.toml` (`edition = "2024"`, `max_width = 100`), `clippy.toml` (`allow-unwrap-in-tests = true`, `allow-expect-in-tests = true`, `allow-print-in-tests = true`), `.gitignore` additions (`/target`, `*.snap.new`, `.pulse/cache`).
  - `.github/workflows/ci.yml`: ubuntu + macos, runs `scripts/check.sh`.
  - `README.md`: 15-line stub (what, status, build instructions).
  - Add dependencies lazily: each task `cargo add`s what it first needs. Here: only `clap` (features `derive`), `anyhow`.
- **Done when**
  - `scripts/check.sh` is green.
  - `cargo run -q -p pulse-cli -- --version` prints `pulse 0.1.0`.
  - `cargo run -q -p pulse-cli -- scan` exits 2 with the stub message.
- **Not now** any real command logic.

## P0.2 — Core model types, Asset & LineIndex
- **Size** M · **Depends** P0.1
- **Read** `scripts/spec.sh "Key types"`, `scripts/spec.sh "Discovery table"` (for the AssetKind/LoadMode lists)
- **Do** (all in `pulse-core`)
  - `model.rs`: `Severity` (Ord, `FromStr`/`Display` lowercase, serde), `Category` (+ `prefix()` → "SEC"…), `AssetKind` (+ `as_str`, `FromStr` in snake_case as used in config), `KindSet` (u16 bitset, const fns, one const per kind such as `KindSet::MCP_CONFIG`, plus `ALL` and `MARKDOWN` = md-based kinds), `LoadMode`, `Span`, `Location`, `Finding`, `RuleMeta`.
  - `rule.rs`: `Rule`, `ProjectRule`, `RuleCtx` (placeholder `options: &toml::Table`-like field may be a generic `RuleOptions` newtype; keep it minimal for now).
  - `text/lines.rs`: `LineIndex::new(&str)` (memchr over `\n`), `fn location(&self, byte) -> (line, col)` 1-based, col counted in chars; binary search. Unit tests: empty file, CRLF, multibyte chars, offset at EOF.
  - `asset.rs`: `Asset { path: Arc<Path>, rel_path: String (forward slashes), kind, load, source: String, lines: LineIndex }` + `Asset::span_to_location(Span) -> Location`. Leave a clearly marked spot for `OnceLock` views (added in P1.x).
  - `Finding::new(meta, asset, span, message)` helper that fills location and default severity.
- **Done when** check is green; unit tests cover LineIndex edge cases, `Severity` ordering/parsing, `KindSet` ops.
- **Not now** parsing views, config.

## P0.3 — Config (`pulse.toml`) & `pulse init`
- **Size** M · **Depends** P0.2
- **Read** `scripts/spec.sh "pulse.toml"`, `scripts/spec.sh "Quality Gate"` (config block only)
- **Do**
  - `pulse-core/src/config.rs`: serde structs mirroring the default file (`Scan`, `CustomAssetDecl`, `Context`, `Budgets`, `Gate`, `RuleSetting` = `Off | Severity(Severity) | Options(table with optional "severity"/"enabled")`, `Ignore`, `CustomRule`). `#[serde(deny_unknown_fields)]` and `Default` impls equal to the spec defaults.
  - `Config::load(root, explicit: Option<&Path>) -> Result<Config, ConfigError>`: missing file → defaults; parse errors carry line/col (from `toml` span).
  - Validation pass: unknown rule IDs are deferred (validated later in the engine, against the registry); custom rule ID regex; `max_context_debt` in [0,1]; budgets > 0.
  - `DEFAULT_TOML: &str` = the exact commented file from the spec (`include_str!` from `crates/pulse-core/src/default_pulse.toml`). Test: parsing `DEFAULT_TOML` yields `Config::default()`.
  - `pulse init [--force]` writes it to `<root>/pulse.toml`, refusing to overwrite without `--force` (exit 2, message).
- **Done when** check is green; tests for defaults equality, a bad key error message, a severity override parse, `init` behavior (tempdir).
- **Not now** applying config to rules (P2.1).
