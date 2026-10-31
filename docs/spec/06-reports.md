# 06 — Reports

All reporters are pure functions `fn render(report: &Report, opts: &RenderOpts) -> String` in `pulse-report`. Output is deterministic: findings are sorted by (path, line, col, rule_id), and no timestamps appear except in JSON `meta`.

## Terminal (default)

```
 Pulse 0.1.0 · 42 assets · 61.2k tokens · 8 ms

 QUALITY GATE   FAILED
   ✖ severity ≥ high        3 issues   (allowed 0)
   ✔ context debt           6.1%       (max 10%)

 CONTEXT LOAD   Always-on context: the tokens each assistant reads on every request
                                          tokens  of 200k window  of 8k budget
   Copilot         ██░░░░░░░░░░░░░░░░░░    18.4k            9.2%          230%
   Claude Code     █░░░░░░░░░░░░░░░░░░░    12.1k            6.1%        151.2%
   Always loaded   CLAUDE.md 8.2k · 40 skill descriptions 6.3k · .github/copilot-instructions.md 3.9k (Copilot)
   On demand       42.8k tokens in skills, agents, prompts and scoped rules, loaded only when used
   Budget          8k always-on tokens per assistant (`budgets.always_on` in pulse.toml). Bars show the share of the 200k context window; they turn yellow above 75% of the budget and red above it.

   Security      E   2 vulnerabilities
   Reliability   B   1 bug
   Clarity       C   5 smells
   Efficiency    B   context debt 6.1% · share of tokens that is avoidable waste (duplicates, filler, over-budget excess)

   CRITICAL 2   HIGH 1   MEDIUM 4   LOW 7   INFO 0
   Fixes shown for medium and higher · run pulse scan -v to see all

 CLAUDE.md
   12:5    CRITICAL  AI-SEC-001  Invisible character U+202E RIGHT-TO-LEFT OVERRIDE
                     ↳ Remove the character.
   40:1    MEDIUM    AI-COG-001  Contradicts line 12: "Always use TypeScript" vs "Use plain JavaScript"
                     ↳ Remove one directive, or scope both ("for tests…", or under headings naming each scope).

 .mcp.json
   7:16    HIGH      AI-SEC-005  Unpinned MCP package "@modelcontextprotocol/server-github" (npx -y)
                     ↳ Pin an exact version (npx pkg@1.2.3, uvx pkg==1.2.3, image@sha256:...).
   …

 ↳ Fixes shown for medium and higher findings. Run with -v to see fixes for all 14 findings, or `pulse explain <RULE-ID>` for a rule's full guidance.
```

- CONTEXT LOAD (ADR-045, ADR-048): what it measures; under a header row, one row per listed assistant (largest first; a single `Every assistant` row when no always-on file names one) with a 20-cell bar of its share of `context.window` (at least one cell when anything loads; down to 10 cells on narrow terminals), green/yellow/red against `budgets.always_on` (see Metrics), its tokens, its share of the window and its share of the budget; the top 4 always-loaded entries of the largest assistant (all skill descriptions as one entry, all agent descriptions as another, then `…`), followed by that assistant, or by every assistant when they load the same; the on-demand tokens when there are any; a `Budget` line saying what the budget is, where it is set, and what the bars and colors mean. With nothing always-on the list reads `nothing: no always-on instructions, skills or agents`.
- SCOPE (only with `--assistant`, ADR-048), above the gate: `<Assistant> only (--assistant) · N assets scanned`, then `Not scanned`: how many files the selection doesn't load, grouped by reason (`for <assistants>` that load them, or `library copies` that no assistant loads from where they are), with `run with -v to list them`; `-v` lists each file with its reason. Only files under `PATHS` / `--changed-since` are counted; `-q` omits the block.
- Colors: gate PASSED green / FAILED red background; ratings A green, B lime, C yellow, D orange, E red; severities colored. No color when not a TTY, with `--color never`, or when `NO_COLOR` is set.
- The remediation line (`↳`) is shown for Medium, High and Critical (Low and Info stay one line each), or for all with `-v`. A rule may give a fix specific to the finding (what is missing, which lines) instead of its default one. When some fixes are hidden (not `-v`, not `-q`), the summary says so under the severity counts and a footer after the findings says how to see them (ADR-044).
- On a terminal, long messages, fixes, the footer and the CONTEXT LOAD / rating detail lines wrap at its width, continuation lines indented to the column where their text starts (ADR-047); output to a pipe or file keeps one line each.
- With `-q`: only the findings, in `path:line:col: SEVERITY RULE message` form (one line each, grep-friendly).
- Locations are relative to the root and use `/` separators on every OS.

## JSON (`--format json`)

Stable, versioned schema (`"schema": "pulse/report@1"`):

```json
{
  "schema": "pulse/report@1",
  "meta": { "version": "0.1.0", "root": ".", "duration_ms": 8, "generated_at": "2026-10-12T10:00:00Z" },
  "gate": { "status": "failed", "conditions": [ { "name": "fail_on_severity", "actual": "critical", "threshold": "high", "passed": false } ] },
  "ratings": { "security": "E", "reliability": "B", "clarity": "C", "efficiency": "B" },
  "metrics": { "assets": 42, "total_tokens": 61230, "always_on_tokens": 18400, "context_load": 0.092,
               "context_load_by_assistant": [ { "assistant": "copilot", "always_on_tokens": 18400, "context_load": 0.092 } ],
               "context_debt": 0.061,
               "issues": { "critical": 2, "high": 1, "medium": 4, "low": 7, "info": 0 } },
  "scope": { "assistants": ["copilot"],
             "not_loaded": [ { "path": ".cursor/rules/style.mdc", "kind": "scoped_instructions", "reason": "other_assistants", "loaded_by": ["cursor"] },
                             { "path": "agents/a.agent.md", "kind": "agent", "reason": "library" } ] },
  "findings": [ { "rule": "AI-SEC-001", "severity": "critical", "category": "security", "path": "CLAUDE.md",
                  "line": 12, "col": 5, "end_line": 12, "end_col": 6, "message": "…", "remediation": "…",
                  "token_waste": 0, "fingerprint": "9f86d081884c7d65", "new": true } ]
}
```

Gate values: severities and ratings are strings, ratios and token counts numbers, "no open finding" is `null`. `metrics.issues` and the gate count every open finding; `--min-severity` only filters `findings`. `category` is `null` for custom rules; `fingerprint` is 16 hex digits; `new` is `false` only for findings in the baseline. `context_load_by_assistant` lists each assistant with always-on assets of its own (`copilot`, `claude_code`, `cursor`, `windsurf`, `cline`, `gemini_cli`, `codex`), largest first, ties in that order; it is `[]` when no asset names one, and its first entry equals `always_on_tokens`/`context_load` (ADR-045). In new-code mode `metrics` gains `new_issues` (same shape as `issues`, after it); otherwise the key is absent. `scope` is present only with `--assistant` (ADR-048): the selection and every asset it left out, sorted by path, with `reason` `other_assistants` (plus `loaded_by`) or `library`. Object keys keep the order above.

## SARIF 2.1.0 (`--format sarif`)

For GitHub Code Scanning. `tool.driver.rules[]` comes from `RuleMeta` (id, name, shortDescription, help.markdown from the embedded docs, `properties.tags = [category]`, `defaultConfiguration.level`). Mapping: Critical/High → `error`, Medium → `warning`, Low/Info → `note`. Also emit `properties.security-severity` (Critical 9.5, High 8.0, Medium 5.5, Low 3.0) for security rules. `partialFingerprints.pulse/v1` = fingerprint. With a baseline, each result carries `baselineState` (`new` or `unchanged`). `rules[]` lists only the rules that have results, in ID order (`ruleIndex` points into it); `help.text` is the remediation; custom rules carry only their `id`. Results use `artifactLocation.uri` relative to the root (percent-encoded, `uriBaseId: "%SRCROOT%"`) and `columnKind: "unicodeCodePoints"`; object keys are sorted. Validated in tests against the vendored schema `crates/pulse-report/tests/schemas/sarif-schema-2.1.0.json` (ADR-034).

## Markdown (`--format markdown`)

For PR comments: a gate badge line, the gate conditions, the context-load block (what it measures, a table with one row per assistant: a 🟢/🟡/🔴 dot against the budget, tokens, share of the window, share of the budget; "Always loaded", "On demand", "Budget" and, with `--assistant`, "Scope" bullets), a ratings table followed by what context debt means, an issue-count table, then findings grouped by file in a collapsible `<details>` per file. Capped at 50 findings, with a "+N more" line. Cells are HTML-escaped with `|` escaped and line breaks folded; `--min-severity` filters the listed findings only.

## GitHub (`--format github`)

GitHub Actions workflow commands, one per finding: `::error file=CLAUDE.md,line=12,col=5,title=AI-SEC-001::Invisible character…`. Critical/High → `error`, Medium → `warning`, Low/Info → `notice`. Then a plain-text gate summary, with the context-load line, one line per assistant (tokens, share of the window, share of the budget), the "Always loaded" / "On demand" / "Budget" lines and, with `--assistant`, a "Scope" line. If `GITHUB_STEP_SUMMARY` is set, the Markdown report is also appended to that file. Messages escape `%`, CR and LF (`%25`, `%0D`, `%0A`); property values also escape `:` and `,` (`%3A`, `%2C`).
