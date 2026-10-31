# 01 — Architecture

## Language and toolchain

Rust, edition 2024, stable toolchain (ADR-001). One static binary, `pulse`.

## Workspace layout

```
Cargo.toml                 # [workspace], shared deps, lints, release profile
crates/
  pulse-core/              # model types, config, discovery, parsing, text utils (no rules)
    src/model.rs           # Severity, Category, AssetKind, LoadMode, Span, Location, Finding, RuleMeta
    src/rule.rs            # Rule + ProjectRule traits, RuleCtx
    src/config.rs          # pulse.toml schema, defaults, validation
    src/discover.rs        # ignore-based walker + globset classification
    src/asset.rs           # Asset, ParsedAsset (lazy views)
    src/parse/markdown.rs  # frontmatter, headings, code fences, HTML comments, links/imports
    src/parse/json.rs      # JSONC with spans, typed views (McpConfig, ClaudeSettings, ToolDefs)
    src/text/lines.rs      # LineIndex: byte offset -> (line, col)
    src/text/normalize.rs  # NFKC, invisible stripping, casefold, offset map back to source
    src/text/tokens.rs     # token estimator
  pulse-rules/             # built-in rules, one module per category
    src/lib.rs             # REGISTRY: &[&dyn Rule], PROJECT_REGISTRY
    src/security/*.rs      # AI-SEC-*
    src/structure/*.rs     # AI-STR-*
    src/efficiency/*.rs    # AI-EFF-*
    src/cognitive/*.rs     # AI-COG-*
    src/lexicon/*.rs       # shared word lists (injection phrases, filler, vague terms, ...)
    src/custom.rs          # declarative custom rules from pulse.toml
  pulse-engine/            # orchestration: discover -> parse -> run rules -> suppress -> baseline -> score -> gate
    src/lib.rs             # scan(&Config, root) -> Report
    src/suppress.rs        # inline + config suppressions
    src/baseline.rs        # fingerprints, baseline file
    src/score.rs           # ratings, context debt, context load
    src/gate.rs            # quality gate evaluation
  pulse-report/            # pure functions Report -> String/bytes
    src/terminal.rs  src/json.rs  src/sarif.rs  src/markdown.rs  src/github.rs
  pulse-cli/               # bin "pulse": clap, commands, exit codes, I/O
docs/rules/AI-XXX-NNN.md   # user-facing rule docs, embedded with include_str! for `pulse explain`
tests/fixtures/            # shared fixture trees (rule fixtures live in crates/pulse-rules/tests/fixtures)
```

Dependency direction: `core <- rules <- engine <- report <- cli` (report depends on engine only for `Report`). `pulse-core` has no dependency on any other workspace crate.

## Data flow

```
config::load(root)
  -> discover::walk(root, &cfg)                      // Vec<DiscoveredFile{path, kind}>, parallel walk
  -> par_iter: read (size cap) -> Asset::new(...)     // bytes + LineIndex, parsed lazily
       -> for rule in REGISTRY where rule.applies_to(kind) && enabled: rule.check(&asset, &ctx, &mut out)
  -> PROJECT_REGISTRY rules over &[Asset]           // cross-asset: budgets, duplicates, cross-file contradictions, name collisions
  -> apply severity overrides -> suppressions -> baseline (new vs existing)
  -> score (ratings, debt, load) -> gate
  -> Report { findings (sorted), metrics, gate, timings }
```

## Key types (sketch; the implementation may refine names, but not semantics)

```rust
pub enum Severity { Info, Low, Medium, High, Critical }           // Ord: Info < ... < Critical
pub enum Category { Security, Structure, Efficiency, Cognitive }  // ID prefixes SEC, STR, EFF, COG
pub enum AssetKind { Instructions, ScopedInstructions, Agent, Prompt, Skill, Tools, McpConfig, Settings, Hooks, Custom }
pub enum LoadMode { Always, Conditional, OnDemand }                // drives context-load metrics

pub struct Span { pub start: u32, pub end: u32 }                  // byte offsets into the asset source
pub struct Location { pub path: Arc<Path>, pub line: u32, pub col: u32, pub end_line: u32, pub end_col: u32 }

pub struct Finding {
    pub rule_id: &'static str,
    pub severity: Severity,          // after config overrides
    pub location: Location,
    pub message: String,
    pub remediation: Option<Cow<'static, str>>,  // RuleMeta's by default; a rule may set a per-finding fix
    pub token_waste: u32,            // > 0 only for efficiency findings; feeds context debt
    pub fingerprint: u64,            // set by the engine
}

pub struct RuleMeta {
    pub id: &'static str, pub name: &'static str, pub category: Category,
    pub default_severity: Severity, pub kinds: KindSet,            // bitset of AssetKind
    pub summary: &'static str, pub remediation: &'static str,
    pub inline_suppressible: bool,   // false for critical SEC rules (ADR-006)
}

pub trait Rule: Sync {
    fn meta(&self) -> &'static RuleMeta;
    fn check(&self, asset: &Asset, ctx: &RuleCtx, out: &mut Vec<Finding>);
}
pub trait ProjectRule: Sync {
    fn meta(&self) -> &'static RuleMeta;
    fn check_project(&self, assets: &[Asset], ctx: &RuleCtx, out: &mut Vec<Finding>);
}
```

`Asset` owns the source (`String`), the path, kind, load mode, `LineIndex`, and `OnceLock` caches for its parsed views (`markdown()`, `json()`, `normalized()`, `tokens()`). Rules share parsed views, so each view is computed at most once per asset.

`RuleCtx` holds the loaded `Config` (`ctx.config`, e.g. `[budgets]` for EFF rules), the per-rule options store (`ctx.options::<T>(rule_id)`: typed, from `[rules.<ID>]`, parsed once per scan, validated up front through `Rule::validate`), the project root and a file-exists oracle (a `HashSet` of walked paths) for reference checks. Oversized/unreadable files become skipped assets that only AI-STR-002 sees (ADR-013).

## Performance design

- **Walk**: `ignore::WalkBuilder::build_parallel()` honors `.gitignore`. Directories are pruned with built-in excludes (`node_modules`, `target`, `.git`, `dist`, `build`, `vendor`, `.venv`). Classification uses one `globset::GlobSet` match per file and allocates nothing for non-assets.
- **Read**: skip files over `scan.max_file_bytes` (default 1 MiB) and report them as Info. Invalid UTF-8 is decoded lossily and reported as AI-STR-002.
- **Parallelism**: `rayon` across assets. Rules run sequentially per asset; assets are small, so the parallelism goes across files.
- **Matching**: keyword lists compile once into `aho_corasick::AhoCorasick` (`LazyLock`). Use the `regex` crate only (linear time, no ReDoS) and compile each pattern once. Byte scans use `memchr`.
- **Allocation**: findings go into a per-thread `Vec` and are merged once. Messages are built only when a finding fires.
- **Startup**: no global initialization beyond lazies. Config parsing happens only if `pulse.toml` exists. `scan` starts building the costly lexicon automatons and regexes (`pulse_rules::warm_up`) on the rayon pool before the walk, so they're ready when the rules run (ADR-039); `pulse --version` and other commands pay nothing.
- **Release profile**: `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`, `strip = true`, `opt-level = 3`.
- **Timings**: `--timings` prints per-phase durations (walk, parse+rules, project rules, report) to stderr.

## Error handling

- Library crates use `thiserror` enums; `pulse-cli` uses `anyhow` for context.
- `unwrap`/`expect` are not allowed outside tests (clippy `unwrap_used`, `expect_used`), except `LazyLock` initialization of compile-time constant patterns, with a `// SAFETY-ish:` comment.
- A malformed asset never aborts a scan. It becomes an AI-STR finding.
- Exit codes: `0` success / gate passed, `1` gate failed (`ci`, or `scan --fail-on-gate`), `2` usage, config or I/O error.

## Dependencies (pinned choices; add with `cargo add`)

| Purpose | Crate |
|---|---|
| CLI | `clap` (derive) |
| Errors | `thiserror`, `anyhow` |
| Serde | `serde`, `serde_json`, `toml` |
| YAML frontmatter | `serde_yaml_ng` |
| JSONC with ranges | `jsonc-parser` |
| Markdown | `pulldown-cmark` (offset iterator) |
| Walk / globs | `ignore`, `globset` |
| Parallelism | `rayon` |
| Matching | `regex`, `aho-corasick`, `memchr` |
| Unicode | `unicode-normalization` |
| Hashing | `xxhash-rust` (xxh3) |
| Terminal | `anstream`, `anstyle`, `terminal_size` |
| Tests | `insta`, `assert_cmd`, `predicates`, `tempfile` |
| Bench | `divan` |

Adding a dependency outside this table needs an entry in `docs/decisions.md`.
