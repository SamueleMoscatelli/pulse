# 02 — Assets & discovery

## Discovery table

Patterns are globset globs relative to the scan root. `**/` means "at any depth". The first match wins, in table order (most specific first).

| # | Pattern(s) | AssetKind | LoadMode | Format |
|---|---|---|---|---|
| 1 | `**/.claude/settings.json`, `**/.claude/settings.local.json` | Settings | Always | JSONC |
| 2 | `**/.cursor/hooks.json`, `**/hooks/hooks.json` | Hooks | Always | JSONC |
| 3 | `**/.mcp.json`, `**/.vscode/mcp.json`, `**/.cursor/mcp.json`, `**/claude_desktop_config.json`, `**/.gemini/settings.json` | McpConfig | Always | JSONC |
| 4 | `**/SKILL.md` | Skill | Always (frontmatter) / OnDemand (body) | MD+FM |
| 5 | `**/.claude/agents/**/*.md`, `**/.github/agents/*.agent.md`, `**/.github/chatmodes/*.chatmode.md` | Agent | Always (description) / OnDemand (body) | MD+FM |
| 6 | `**/.claude/commands/**/*.md`, `**/.github/prompts/*.prompt.md`, `**/*.prompty` | Prompt | OnDemand | MD+FM |
| 7 | `**/.cursor/rules/**/*.mdc`, `**/.claude/rules/**/*.md`, `**/.github/instructions/**/*.instructions.md`, `**/.windsurf/rules/**/*.md`, `**/.clinerules/**/*.md` | ScopedInstructions | Conditional* | MD+FM |
| 8 | `**/CLAUDE.md`, `**/CLAUDE.local.md`, `**/AGENTS.md`, `**/GEMINI.md`, `**/.github/copilot-instructions.md`, `**/.cursorrules`, `**/.windsurfrules`, `**/.clinerules` (file) | Instructions | Always** | MD |
| 9 | `**/*.tools.json`, `**/tools/*.json` (content-sniffed, see below) | Tools | Always | JSON |
| 10 | `[[scan.custom]]` entries in `pulse.toml` | Custom (declared kind) | declared | declared |

\* ScopedInstructions become **Always** when their frontmatter says so: `.mdc` with `alwaysApply: true`; `.claude/rules` and `.github/instructions` without `paths`/`applyTo`, or with `applyTo: "**"`.
\** Nested `CLAUDE.md`/`AGENTS.md` below the root load only when the agent works in that subtree. They count as **Conditional** for context-load metrics; root-level ones count as **Always**.

### Content sniffing for Tools

A `*.json` file under a `tools/` directory is a Tools asset only if its top-level value is an object or array whose element(s) have `name` plus (`input_schema` | `parameters` | `inputSchema`). Otherwise it is ignored silently. Sniffing parses at most the first 64 KiB; if that prefix is truncated JSON, it falls back to a key scan (starts with `{`/`[`, contains `"name"` and a schema key). `*.tools.json` is not sniffed.

### Default excludes

`.git`, `node_modules`, `target`, `dist`, `build`, `out`, `vendor`, `.venv`, `venv`, `__pycache__`, `.next`, `.turbo`, `coverage`. `.gitignore`, `.ignore` and `.pulseignore` are honored. Hidden directories are **not** skipped, since most assets live in them. `scan.exclude` / `scan.include` in `pulse.toml` extend the table.

Matching order: built-in rows 1–9, then `[[scan.custom]]`, then `scan.include` globs (kind Custom, load OnDemand). Ignore files are read from the scan root downward only: `.gitignore` applies even outside a git repo, while ancestors' ignore files and the global gitignore do not (deterministic across machines). `scan.exclude` globs are relative to the root and `*` does not cross `/`; use `**/` for any depth. Unfollowed symlinks are recorded as existing paths, not analyzed. `.claude/CLAUDE.md` counts as root-level. Skill and Agent are discovered with load Always; only their description counts toward always-on tokens.

## Per-kind parsed views

| Kind | Views available to rules |
|---|---|
| Instructions, ScopedInstructions, Agent, Prompt, Skill | `markdown()`: frontmatter (YAML mapping + span + key line map), body span, headings (level, text, span), code fences (lang, span), HTML comments (span), links/images (dest, span), `@path` imports (CLAUDE.md style, span), paragraphs (span) |
| McpConfig | `mcp()`: servers map → `{ name, transport: Stdio{command,args,env} | Http{url,headers} | Sse{url,headers}, span }`. Handles `mcpServers` (Claude/Cursor/Gemini) and `servers` + `inputs` (VS Code) |
| Settings | `settings()`: `permissions.{allow,deny,ask}` with spans, `hooks` (event → matchers → commands with spans), `env`, misc flags (`enableAllProjectMcpServers`, `defaultMode`) |
| Hooks | `hooks()`: same hook model as Settings |
| Tools | `tools()`: list of `{ name, description, schema, span }` |
| all | `source()`, `line_index()`, `normalized()`, `tokens()` |

Every JSON view keeps a JSON-pointer → `Span` map, so rules can report exact lines for nested keys.

JSON view details (ADR-012): `json()` is JSONC (comments, trailing commas) as an owned span tree; the pointer map is built on first use. MCP servers carry `args`/`env`/`headers` beside a `Transport` (`Stdio`/`Http`/`Sse`/`Unknown`). Hooks assets reuse the Settings view. Shape problems are `ViewError { span, msg }` lists on each view; syntax errors live on `json()`.

Markdown view details (ADR-011): frontmatter needs a closing `---` line; `@path` imports skip code, raw HTML, links, emails and bare URLs. ScopedInstructions promotion (Discovery table, footnote *) is resolved by `Asset::effective_load()`.

## Token estimation (ADR-005)

`tokens(text) = round(max(chars / 4.0, words * 1.3) * calibration)`, where `chars` counts Unicode scalar values, `words` counts runs of non-whitespace (ASCII whitespace separates words, ADR-010), and `calibration` comes from `pulse.toml` (default `1.0`). Fenced code counts characters at 3.2 chars/token. The estimator must process ≥ 1 GB/s in the benchmark (single pass, no allocation).

For Skill and Agent, `Always` tokens = frontmatter `name` + `description` only. The body is OnDemand.

## Text normalization

`normalize(src) -> Normalized { text: String, map: Vec<u32> }`:
1. Drop invisible and format characters (the AI-SEC-001 set) and tag characters U+E0000–U+E007F.
2. NFKC (folds full-width and compatibility forms).
3. Map common confusables for Latin letters (Cyrillic/Greek lookalikes → ASCII) using a small built-in table.
4. Lowercase, collapse whitespace runs into one space, strip Markdown emphasis markers `*_~\``.

`map[i]` is the source byte offset of normalized byte `i` (plus a trailing sentinel equal to the source length; NFKC runs per starter segment, see ADR-010), so findings on normalized text report source locations. Injection, exfiltration, filler and contradiction rules match on normalized text, which makes simple obfuscation useless.
