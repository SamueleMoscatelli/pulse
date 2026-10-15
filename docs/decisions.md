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

## ADR-008 — Windows in CI from Phase 0 (2026-10-12)
Context: P0.1 specified CI on ubuntu + macos only, but Phase 7 ships a Windows x64 binary.
Decision: the CI matrix includes `windows-latest`, running `scripts/check.sh` under Git Bash. `.gitattributes` forces LF checkout so fmt, snapshots and fixtures behave the same on every OS; CRLF test input is generated in code.

## ADR-009 — Discovery clarifications (2026-10-12)
Context: P1.1 found gaps in 02-assets: the kind of `scan.include` matches, custom vs built-in priority, and ignore-file scope.
Decision: built-in rows win, then `[[scan.custom]]`, then `scan.include` (Custom, OnDemand). Ignore files apply from the scan root down (`require_git(false)`, `parents(false)`); ancestors' and the global gitignore are ignored for cross-machine determinism. Unfollowed symlinks count as existing paths but are not analyzed. `.claude/CLAUDE.md` is root-level (Always). Truncated `tools/*.json` sniffing falls back to a key scan.
Consequences: a custom glob cannot reclassify a built-in asset; users who rely on a global gitignore must use `.pulseignore` or `scan.exclude`.

## ADR-010 — Text utility clarifications (2026-10-12)
Context: P1.2 found gaps in "Token estimation" and "Text normalization" (02-assets): what counts as a word separator, how NFKC interacts with the offset map, and how a normalized range maps back to an end offset.
Decision: the token estimator splits words on ASCII whitespace only (byte loop, no decoding). Normalization runs NFKC per segment (a starter plus its trailing non-starters), so every output byte maps to the segment's first source byte; Hangul jamo composition across starters is not applied. `map` carries a trailing sentinel equal to the source length, and `source_span` extends the end to the next source unit, so dropped characters after a match fold into its span.
Consequences: exotic Unicode spaces don't split words for token estimates; source spans may include trailing invisibles, which is harmless for reporting.

## ADR-011 — Markdown view clarifications (2026-10-12)
Context: P1.3 found gaps in "Per-kind parsed views" (02-assets): unterminated frontmatter, `@import` edge cases, key-line map type, and where frontmatter promotion lives.
Decision: a `---` opener with no closing `---` line is not frontmatter (the file is all body). Frontmatter that is valid YAML but not a mapping is `Err`; YAML error messages drop the YAML-relative `at line N column M` marks, and `line` is file-relative. Alias expansion relies on serde_yaml_ng's repetition limit, which surfaces as `Err`. `key_lines` is a `BTreeMap` (deterministic iteration). `@path` imports are skipped inside code, raw HTML and links, and after a word char, `/`, `:` or `=` (emails, bare URLs, attributes); trailing dots are trimmed. ScopedInstructions promotion is computed by `Asset::effective_load()` (discovery keeps the path-only mode) and applies only when the discovered mode is Conditional; invalid YAML keeps Conditional. `Asset::with_calibration` sets the factor for `tokens()`.
Consequences: an unclosed frontmatter is not reported as a YAML error. A `@handle` mention in prose still counts as an import (as the spec's pattern implies).

## ADR-012 — JSON view clarifications (2026-10-12)
Context: P1.4 found gaps in "Per-kind parsed views" (02-assets): `jsonc-parser`'s AST borrows the source (an `Asset` can't hold it), the MCP transport shape, hook files that aren't Claude settings, and the cost of a full pointer index.
Decision: `JsonDoc` converts the AST into an owned `JsonNode` tree with byte spans (lenient `ParseOptions::default()`; a syntax error is one `ViewError`, and the root is `None`). Pointer lookups (`span_of`, `key_span_of`) walk the tree; the full pointer → span maps (`pointer_spans()`) are built only on first use. `McpServer` keeps `args`/`env`/`headers` flat beside a `Transport` enum (`Stdio{command}`, `Http{url}`, `Sse{url}`, `Unknown`); `httpUrl` (Gemini) and `serverUrl` (Windsurf) are URLs, and an explicit `type` without its endpoint is a shape error. Hooks assets reuse `SettingsView` (`Asset::hooks()`); Cursor's flat `{command}` entries become a matcher-less `HookEntry`. Hook types other than `command`/`prompt` (`http`, `agent`, …) have an empty command whose span covers the hook. Tools are read from a root array or a root `tools` key, in Anthropic, OpenAI-nested and OpenAI-flat forms. Duplicate keys: the last one wins.
Consequences: views report only shape errors; AI-STR-002 reads syntax errors from `Asset::json().errors`.

## ADR-013 — Engine pipeline clarifications (2026-10-12)
Context: P2.1 found gaps in "Data flow" and "Key types" (01-architecture): where per-rule options are validated, how skipped files reach AI-STR-002, which assets project rules see under a path filter, and what "always-on" means per kind.
Decision: `RuleCtx.options` is an `OptionsStore` (all `[rules.<ID>]` tables; typed parses cached per (rule, type)); `Rule`/`ProjectRule` gain a default `validate(&ctx)` that the engine calls once per enabled rule before any check, so bad options abort the scan as `EngineError::Config`. Oversized or unreadable files become `Asset::skipped(reason)` (empty source) and only AI-STR-002 runs on them; invalid UTF-8 is decoded lossily with `Asset.lossy = true`. With `ScanOptions.paths`, asset rules run only on matching assets, project rules see every asset, and all findings are filtered by path; `assets_meta` always lists every asset. A `[rules]` severity override applies to every finding of the rule (the Info "skipped" finding included). Dedup merges across rules only: per (location, category) it keeps the rule with the highest-severity finding (ties to the lowest rule ID) and all of that rule's findings there; custom-rule findings are never merged. Files are read with one bounded read (`max_file_bytes + 1`), and rule options are validated before discovery. `Asset::always_tokens()`: the frontmatter `description` for skills/agents, 0 for MCP configs, settings and hooks, the whole file for other Always assets. `Report.walk_errors` carries the walker's unreadable entries.
Consequences: path-filtered CI scans keep project-wide budgets meaningful. Overriding AI-STR-002's severity also raises skipped-file notices.

## ADR-014 — AI-SEC-001 clarifications (2026-10-13)
Context: P2.2 found gaps in "AI-SEC-001" (03-rules) and "Rule fixture harness" (07-testing): message format for mixed and tag runs, which emoji bases the guards accept, and the harness demanding a committed `fail_*` file while invisible content must be generated in code.
Decision: one finding per contiguous run; `(×N)` only when N > 1, `and others` when the run mixes code points; runs containing tag characters report `Hidden tag text (×N): "<payload>"` (printable tags only, 80-char cap). The ZWJ guard needs non-ASCII emoji on both sides; a variation selector is allowed after any emoji base, ASCII keycap bases included, and an allowed selector or ZWJ keeps the emoji sequence going. The pre-filter is `memchr3` + `memchr2` over the five lead bytes, then a char walk from the first hit. A `_generated` file in a fixture dir stands in for `fail_*` when all fail cases are generated in `fixtures.rs`.
Consequences: emoji tag sequences (subdivision flags) are still reported; tracked in the Backlog. Tag-run messages can slightly exceed the 100-char guideline.

## ADR-015 — AI-COG-001 clarifications (2026-10-14)
Context: P2.5 found gaps in "AI-COG-001" (03-rules): how to cache directives, what "skip inline code" means for objects, how "same object" compares objects of different lengths, and when a heading "names a scope".
Decision: `cognitive::directives::extract(asset)` is a pure function that each rule calls; nothing is cached, because AI-COG-001 runs once per asset and AI-COG-005 will extract from each asset once. If AI-COG-005 compares one asset against several others, it should extract each asset once into a local map rather than add a cache to `Asset`. `Prepared` precomputes each directive's section scope and exclusive-set members, so the pairwise loop does no allocation or lexicon scans. Code blocks, HTML comments and headings are skipped; inline code only blocks sentence splitting, and its text stays in the sentence (`Never use \`any\``). A `do` directly followed by a negator opens a negative directive anywhere in the sentence ("We do not use yarn"). The verbs use/using/prefer/avoid and verb-less markers (`No semicolons`) count as the verb `use`. A directive's context is the content words after its first qualifier (`for`, `in`, `when`, …) outside the verb and object, and two directives with different contexts never conflict. For opposite polarity, the negative object must be a prefix of the positive object, so "Never use semicolons" contradicts "Always use semicolons at the end", but "Never use TypeScript enums" only narrows "Always use TypeScript". The section guard applies only when both nearest headings name scopes, meaning at least one word is not in `GENERIC_HEADINGS`, and the headings differ. Each directive gets at most one finding, against the first earlier directive it conflicts with. Quotes are cut to 34 chars so the message stays within 100.
Consequences: a file-title heading such as "# Acme assistant" names a scope, so a directive above the first `##` is not compared with directives in scoped `##` sections. Same-polarity duplicates and paraphrases ("Never use X" / "Avoid X" are equal) produce no finding.

## ADR-016 — Suppression clarifications (2026-10-15)
Context: P2.6 found gaps in "Inline suppressions" (05-cli-config): malformed comments, stacked next-line comments, multi-line findings, the order relative to dedup, and whether `ignore.rules` is validated.
Decision: a suppression matches findings whose start line is its target. A next-line comment skips blank lines and lines that hold only another suppression comment. A comment that lists no IDs or `all`, or a `pulse-ignore-file` after the first heading, becomes an Info AI-STR-002 `invalid suppression: …` finding. Unused-suppression notes name the ID and are emitted only for rules that ran, so a rule disabled by config or `--only`/`--skip` doesn't produce false notes. All notes are skipped when AI-STR-002 is off. Suppression runs after severity overrides and before dedup, so suppressing one rule never hides another rule's finding on the same span. `[[ignore]]` runs after the inline pass and may drop notes. Unknown rule IDs in `ignore.rules` are config errors, like `[rules]`. An unknown ID in an inline comment becomes `invalid suppression: unknown rule <ID>`, except for `X-` IDs, which are left to custom rules. Next-line targets are resolved in one reverse pass over the lines, so hostile files full of stacked comments stay linear. `Report.suppressed` counts every dropped finding, notes included.
Consequences: a multi-line finding is suppressed only from its first line. Custom-rule suppressions are never reported as unused until custom rules run.

## ADR-017 — Scoring & gate clarifications (2026-10-15)
Context: P2.7 found gaps in 04-scoring-gate: how custom rules map to ratings, what "overlapping waste" means for findings that carry locations rather than spans, and how disabled gate conditions are shown.
Decision: a finding's category comes from its `AI-<PREFIX>-` rule ID, so custom `X-` rules have no category and affect no rating (they still count in `fail_on_severity`). Waste overlaps when two EFF findings in one file have intersecting half-open location ranges; each overlapping cluster contributes its max waste. Disabled gate conditions are omitted from `Gate.conditions`. Metrics cover in-scope assets only. Condition values are typed (`GateValue`) and formatted by the reporters.
Consequences: a project of only custom-rule findings rates A everywhere but can still fail the gate through `fail_on_severity`.

## ADR-018 — CLI flag details for `scan`/`ci` (2026-10-15)
Context: P2.8 wired `scan`/`ci`; the Commands section left `-v`, `PATHS` resolution and color precedence implicit.
Decision: `-v, --verbose` is a global flag (remediation for every finding in the terminal report). `PATHS` are root-relative (absolute paths under the root are stripped). An explicit `--color` wins over `NO_COLOR`; `ci` treats `--color auto` as off. `init` uses the global `-C`. Formats without a reporter yet (`json`, `sarif`, `markdown`) exit 2 with "not implemented yet"; `github` renders as terminal until P6.2.
Consequences: `pulse -C dir scan sub/` scans `dir/sub`, not `./sub`; `--new-only`/`--changed-since` are added with their implementation in P6.3.

## ADR-019 — v0.1.0 performance numbers miss the budget on Intel hardware (2026-10-15)
Context: P2.9 ran `scripts/bench.sh` on the only machine available, a 2019 Intel i5-8257U MacBook Pro. The budgets in 00-overview are defined on Apple M-series. Medians: startup 6.1 ms (budget 2), typical 45.7 ms (20), large 316.7 ms (150). Peak RSS (8.2 MB) and binary size (2.8 MB) are within budget.
Decision: v0.1.0 ships with these numbers, and the budgets stay unchanged. On this machine the gap is in the hardware and the OS, not in Pulse: spawning `/usr/bin/true` takes 4.2 ms, and `rg --files` (the same parallel `ignore` walker) needs 40 ms and 380 ms to list the two trees, which is slower than a full Pulse scan of them. Discovery takes about 80% of scan time (`--timings`: discover 28 ms of 37 ms on typical, 190 ms of 225 ms on large). The Perl timing fallback in `bench.sh` replaces a `/usr/bin/time` loop, because `/usr/bin/time` reports only 10 ms resolution.
Consequences: P7.1 must re-measure on Apple M-series hardware (or a `macos-14` arm64 CI runner) before deciding whether the walk needs work. `bench.sh` exits 1 on this machine until then.

## ADR-020 — AI-SEC-003 generic pattern and placeholder guards (2026-10-15)
Context: P3.1 implemented AI-SEC-003. The spec's generic regex `(api[_-]?key|token|secret|password)\s*[:=]…` misses JSON keys (`"apiKey": "…"`, the closing quote sits before the colon) and suffixed keys (`aws_secret_access_key = …`), and its placeholder list has no entry for environment lookups other than `${…}`.
Decision: the generic key may carry a `[a-z_]*` suffix and a closing quote before `:`/`=`, with only spaces or tabs around the separator; matching is case-insensitive and has no leading word boundary (so `OPENAI_API_KEY=` matches). Placeholders also cover `$VAR`, `{{…}}`, `placeholder`, `redacted`, `process.env`, `environ`, `getenv`; "all-same-char" is read as "one character makes up at least half of the value". Two shape guards beyond the spec: an OpenAI match needs upper case, lower case and a digit (kebab-case slugs like `sk-learn-…` are not keys), and a generic value needs a digit and must not look like code (calls, indexing, dotted paths such as `settings.SECRET_KEY`). Overlapping matches keep the earliest start, then the first pattern in `lexicon/secrets.rs` order (Anthropic before OpenAI, providers before generic). The finding's span is the secret value only.
Consequences: AI-SEC-004 must report on the value span (without quotes) so the engine's same-span dedup lets AI-SEC-003 win.
