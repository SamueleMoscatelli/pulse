# Phase 2 — Engine & MVP (v0.1.0)

Goal: `pulse scan` / `pulse ci` work end to end with the four MVP rules from the original draft, plus scoring, gate and the Sonar-style terminal report. At the end of this phase Pulse is usable.

## P2.1 — Engine pipeline, rule registry, fixture harness, AI-STR-002
- **Size** L · **Depends** P1.4
- **Read** `scripts/spec.sh "Data flow"`, `scripts/spec.sh "Key types"`, `scripts/spec.sh "Rule fixture harness"`, `scripts/spec.sh "AI-STR-002"`; skill `add-rule`
- **Do**
  - `pulse-rules/src/lib.rs`: `pub static RULES: &[&dyn Rule]`, `pub static PROJECT_RULES: &[&dyn ProjectRule]`, `fn all_meta() -> impl Iterator<Item=&'static RuleMeta>`, `fn find(id)`.
  - `pulse-engine`: `scan(root, &Config, &ScanOptions) -> Result<Report, EngineError>`: discover → read files (size cap → Info skipped finding) → `par_iter` assets → run enabled asset rules filtered by `KindSet` → project rules → severity overrides from config → dedup (same span + same category: keep the highest severity; AI-SEC-003 over AI-SEC-004) → fingerprints → sort. `Report { findings, assets_meta (rel_path, kind, load, tokens, always_tokens), timings }`. Validate rule IDs in `[rules]` against the registry (unknown → `EngineError::Config` with a suggestion).
  - `ScanOptions { paths filter, only_rules, skip_rules (prefix match) }`.
  - `RuleCtx::options::<T: DeserializeOwned + Default>(rule_id) -> Result<T, ConfigError>`: typed per-rule options from `[rules.<ID>]`, parsed once per scan and cached; unknown keys are errors (rules use `#[serde(deny_unknown_fields)]`).
  - Rule AI-STR-002: emits findings from Markdown/JSON view errors, invalid UTF-8 (Asset gets a `lossy: bool` flag), and skipped files.
  - Fixture harness `crates/pulse-rules/tests/fixtures.rs` per spec, including the completeness assertions (fixtures + `docs/rules/<ID>.md` for every registered rule).
  - Write `docs/rules/AI-STR-002.md` using the template in the `add-rule` skill.
- **Done when** check is green; harness runs AI-STR-002 fixtures (broken JSON, broken YAML frontmatter, valid files); an engine test scans `tests/fixtures/repos/mixed` and asserts deterministic output across 10 runs.
- **Not now** suppressions, scoring, CLI wiring.

## P2.2 — AI-SEC-001 invisible/bidi Unicode
- **Size** S · **Depends** P2.1
- **Read** `scripts/spec.sh "AI-SEC-001"`; skill `add-rule`
- **Do** Implement per spec, reusing `text::unicode`. Group contiguous runs; decode tag-character payloads. Fixture content with invisible characters is generated in tests (see the harness note); committed fixtures cover pass cases (emoji ZWJ family 👨‍👩‍👧, BOM at offset 0).
- **Done when** check is green; tests cover RLO, ZWSP run (×3 count), tag smuggling payload decoding, ZWJ emoji pass, BOM pass, and a mid-file BOM fail.

## P2.3 — AI-SEC-002 prompt-injection phrases
- **Size** M · **Depends** P2.2
- **Read** `scripts/spec.sh "AI-SEC-002"`, `scripts/spec.sh "Text normalization"`; skill `add-rule`
- **Do** Create `pulse-rules/src/lexicon/mod.rs` + `injection.rs` (≥ 40 phrases with variant expansion at build time into one `LazyLock<AhoCorasick>`, `MatchKind::LeftmostLongest`, ASCII case-insensitive on normalized text). Map matches back to source spans via `Normalized::source_span`. Implement the code-fence / security-section downgrade guards and the config `allow` list.
- **Done when** check is green; fixtures: plain phrase, full-width obfuscation, Cyrillic homoglyph, zero-width-split phrase (generated), phrase in a `text` fence (Low), phrase under a "Prompt injection examples" heading (Low), benign text containing "ignore" (pass).

## P2.4 — AI-EFF-001 & AI-EFF-002 token budgets
- **Size** M · **Depends** P2.1
- **Read** `scripts/spec.sh "AI-EFF-001"`, `scripts/spec.sh "AI-EFF-002"`, `scripts/spec.sh "Token estimation"`, `scripts/spec.sh "Metrics"`; skill `add-rule`
- **Do** AI-EFF-001 (asset rule, budget by kind from `ctx.config.budgets`, Medium within 125%) with `token_waste`. AI-EFF-002 (project rule) sums Always tokens using `Asset::always_tokens()` (Skill/Agent → description only; ScopedInstructions per effective load); points at the largest contributor and lists the top 5 in the message. `@imports` are not yet resolved (P4.3 adds them); leave a `// P4.3:` note.
- **Done when** check is green; fixtures generate large content programmatically (no giant committed files); tests for the boundary (budget, budget+1, 125%).

## P2.5 — AI-COG-001 contradictory directives
- **Size** L · **Depends** P2.3
- **Read** `scripts/spec.sh "AI-COG-001"`; skill `add-rule`
- **Do** Shared module `pulse-rules/src/cognitive/directives.rs`: sentence/list-item splitter over the Markdown body (skip code fences/inline code), `Directive { polarity, verb, object: SmallVec<[&str;4]>/String, span, section_heading }`, extracted once per asset and cached (an `OnceLock` keyed in a side map, or computed inside the rule; keep it reusable by AI-COG-005). `lexicon::EXCLUSIVE_SETS` (~15 sets). Implement the opposite-polarity and exclusive-set detection plus the scope guards. Config option `exclusive`.
- **Done when** check is green; fixtures: always/never same object (fail), TypeScript vs JavaScript (fail), tabs vs spaces in different sections "Python"/"Makefile" (pass), conditional "use npm for scripts, pnpm for the workspace" (pass), list-item directives (fail).
- **Not now** cross-file (P5.5).

## P2.6 — Suppressions (inline + config)
- **Size** M · **Depends** P2.5
- **Read** `scripts/spec.sh "Inline suppressions"`, `scripts/spec.sh "pulse.toml"` (`[[ignore]]` part)
- **Do** `pulse-engine/src/suppress.rs`: parse suppression comments from `markdown().html_comments`; next-line / same-line / file scope; enforce `inline_suppressible = false` with an Info note; `[[ignore]]` path globs + rule lists; unused-suppression Info finding (AI-STR-002). Suppressed findings are dropped from `Report.findings` but counted in `Report.suppressed`.
- **Done when** check is green; tests for each form, the ADR-006 behavior, the unused-suppression finding, and a config ignore.

## P2.7 — Scoring, ratings & quality gate
- **Size** M · **Depends** P2.6
- **Read** `scripts/spec.sh "Ratings"`, `scripts/spec.sh "Metrics"`, `scripts/spec.sh "Quality Gate"`
- **Do** `score.rs` (`Metrics`, `Ratings`, context debt with overlapping-waste dedup) and `gate.rs` (`Gate`, `Condition`) per spec; attach both to `Report`. Pure functions with table-driven tests.
- **Done when** check is green; tests: rating thresholds at each boundary, debt overlap dedup, gate pass/fail for each condition type, `fail_on_severity = "none"`.

## P2.8 — Terminal reporter, `scan` & `ci` commands
- **Size** L · **Depends** P2.7
- **Read** `scripts/spec.sh "Terminal (default)"`, `scripts/spec.sh "Commands"`
- **Do** `pulse-report/src/terminal.rs` (anstyle colors, layout per mockup, `-q` mode, `-v` remediation for all, relative `/` paths); wire `scan`, `ci` (format default logic; `github` format falls back to terminal until P6.2), `--fail-on-gate`, `--min-severity`, `--rule/--skip-rule`, `--timings`, `-C`, `-c`, `--color`, exit codes. Use a locked `BufWriter` on stdout.
- **Done when** check is green; `insta` snapshots (color off) for: clean repo (PASSED), mixed repo (FAILED), `-q` mode; `cargo run -q -p pulse-cli -- scan -C tests/fixtures/repos/mixed` looks like the mockup (paste the output into your summary).

## P2.9 — End-to-end tests, benchmarks, v0.1.0
- **Size** M · **Depends** P2.8
- **Read** `scripts/spec.sh "Performance budget"`, `scripts/spec.sh "Performance checks"`
- **Do**
  - `pulse-cli/tests/cli.rs` (`assert_cmd`): exit codes 0/1/2, `init` then `scan`, `ci` failing on a critical, unknown rule in config → 2.
  - `divan` benches in `pulse-engine/benches/scan.rs` (estimator, normalize, full scan over a generated 1,000-asset tree in a tempdir).
  - `scripts/bench.sh` per spec. Run it and record numbers in `docs/perf.md` (machine, date, numbers).
  - Dogfood: `cargo run --release -p pulse-cli -- scan` on this repo. Fix any true findings in our own `.claude/` assets; add `[[ignore]]` entries with reasons for intended ones (e.g. docs quoting injection phrases).
  - Bump nothing: version stays 0.1.0; create the git tag `v0.1.0` only if the user confirms.
- **Done when** check is green; bench numbers within budget (or a decision entry explaining the gap plus a P7.1 note).
