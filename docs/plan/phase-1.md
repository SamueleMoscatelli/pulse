# Phase 1 — Discovery & parsing

Goal: from a directory to a `Vec<Asset>` with cheap, lazily computed parsed views. Everything here is hot-path code, so watch allocations.

## P1.1 — Asset discovery (walker + classification)
- **Size** M · **Depends** P0.3
- **Read** `scripts/spec.sh "Discovery table"`, `scripts/spec.sh "Content sniffing"`, `scripts/spec.sh "Default excludes"`, `scripts/spec.sh "Performance design"`
- **Do**
  - `pulse-core/src/discover.rs`: `Classifier` built once from the table + `scan.include`/`scan.custom` (one `GlobSet`, index → (kind, load)); `discover(root, &Config) -> Result<Discovery>` where `Discovery { assets: Vec<DiscoveredFile { path, rel_path, kind, load }>, all_paths: HashSet<String> /* rel paths of every walked file, for STR-003 */ , skipped: Vec<(path, reason)> }`.
  - `ignore::WalkBuilder` parallel walk: hidden = false (don't skip dotdirs), git_ignore on, custom ignore filename `.pulseignore`, default excludes via `filter_entry`, `follow_links` from config.
  - `.clinerules` can be a file (Instructions) or a dir (ScopedInstructions): handle both.
  - Tools sniffing (cheap prefix parse); ScopedInstructions → Always promotion happens later (needs frontmatter): leave a `TODO(P1.3)` marker in the load-mode resolution function, and record root-vs-nested for CLAUDE.md/AGENTS.md (nested → Conditional).
  - Output sorted by `rel_path` (determinism).
  - Fixture tree `tests/fixtures/repos/mixed/` with ≥ 1 file per table row, plus `node_modules/x/CLAUDE.md` (must be excluded) and a `.gitignore`d asset.
- **Done when** check is green; a test asserts the exact `(rel_path, kind, load)` list for `mixed/`.
- **Not now** reading file contents (beyond sniffing).

## P1.2 — Text utilities: token estimator & normalization
- **Size** M · **Depends** P0.2
- **Read** `scripts/spec.sh "Token estimation"`, `scripts/spec.sh "Text normalization"`, `scripts/spec.sh "AI-SEC-001"` (code-point set)
- **Do**
  - `text/tokens.rs`: `estimate(text, calibration) -> u32` single pass, no allocation; `estimate_markdown(text, code_spans: &[Span], calibration)` applying the code ratio. Tests with known samples (± tolerance).
  - `text/unicode.rs`: `is_invisible(char) -> Option<&'static str /*name*/>` covering the AI-SEC-001 set (shared with that rule), plus `is_emoji_base` (approximate ranges) for the ZWJ guard.
  - `text/normalize.rs`: `normalize(&str) -> Normalized { text, map }` per spec; `Normalized::source_span(norm_range) -> Span`. Small confusables table (Cyrillic а, е, о, р, с, у, х, і; Greek ο, α…).
  - Tests: offsets map back correctly through NFKC expansion/contraction, invisible removal, whitespace collapsing; `"ｉｇｎｏｒｅ"` → `"ignore"`; `"іgnore"` (Cyrillic і) → `"ignore"`.
- **Done when** check is green; tests above pass.
- **Not now** benches (P2.9).

## P1.3 — Markdown view (frontmatter, structure, links, imports)
- **Size** L · **Depends** P1.1, P1.2
- **Read** `scripts/spec.sh "Per-kind parsed views"`, `scripts/spec.sh "Discovery table"` (the ScopedInstructions promotion footnote)
- **Do**
  - `parse/markdown.rs`: `MarkdownView { frontmatter: Option<Frontmatter { span, body_span, value: Result<serde_yaml_ng::Mapping, YamlError{line,msg}>, key_lines: HashMap<String,u32> }>, body: Span, headings, code_blocks { lang, span, lines }, html_comments, links { dest, is_image, span }, imports { path, span }, paragraphs }`.
  - Frontmatter: only if the file starts with `---\n` (allow BOM/CRLF); find the closing `---`. `key_lines` from a line scan of top-level keys.
  - Body: `pulldown_cmark::Parser::new_ext(..).into_offset_iter()` with tables + strikethrough; offsets adjusted by the body start.
  - `@imports`: `@path` tokens outside code (spans and inline code) matching `@(\.{0,2}/)?[\w./-]+` at a word boundary; skip emails.
  - `.cursorrules`/`.windsurfrules` are parsed as Markdown, even if plain text.
  - Wire into `Asset`: `asset.markdown() -> &MarkdownView` via `OnceLock`; `asset.tokens()`, `asset.normalized()` lazies too.
  - Resolve load-mode promotion for ScopedInstructions (alwaysApply / missing `paths`/`applyTo`) in `Asset::effective_load()`; remove the TODO from P1.1.
- **Done when** check is green; unit tests over a rich sample (frontmatter + CRLF + code fences + comments + imports + links) assert spans/lines; an invalid YAML frontmatter yields `Err` with the correct line.
- **Not now** rules.

## P1.4 — JSON views (MCP, settings/hooks, tools)
- **Size** L · **Depends** P1.3
- **Read** `scripts/spec.sh "Per-kind parsed views"`
- **Do**
  - `parse/json.rs`: parse with `jsonc-parser` (comments + trailing commas allowed) into its AST with ranges; a generic `JsonDoc { root, pointer_spans: HashMap<String, Span> }` (pointer → value span; also a key-span map).
  - Typed views built from the AST (not serde, so spans are kept): `McpView { servers: Vec<McpServer { name, name_span, transport, env: Vec<(k, v, span)>, headers, args: Vec<(String, Span)>, span }> }` handling `mcpServers` and VS Code `servers`/`inputs`; `SettingsView { allow, deny, ask: Vec<(String, Span)>, default_mode, enable_all_project_mcp, hooks: Vec<HookEntry { event, event_span, matcher, matcher_span, hooks: Vec<HookCmd { kind, command, command_span, timeout }> }>, env }`; `ToolsView { tools: Vec<ToolDef { name, description, desc_span, schema_span }> }`.
  - Shape errors are collected as `Vec<ViewError { span, msg }>` (consumed by AI-STR-002 in P2.1); never panic.
  - `Asset::json()`, `mcp()`, `settings()`, `tools()` lazies.
  - Fixtures: Claude `.mcp.json`, VS Code `mcp.json` with inputs + comments, `.claude/settings.json` with hooks and permissions, an Anthropic tools array, an OpenAI tools array.
- **Done when** check is green; tests assert server/hook extraction and exact lines for a nested `env` value and a hook `command`.
- **Not now** schema validation (P4.2).
