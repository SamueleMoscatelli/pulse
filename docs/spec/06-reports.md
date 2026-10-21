# 06 — Reports

All reporters are pure functions `fn render(report: &Report, opts: &RenderOpts) -> String` in `pulse-report`. Output is deterministic: findings are sorted by (path, line, col, rule_id), and no timestamps appear except in JSON `meta`.

## Terminal (default)

```
 Pulse 0.1.0 · 42 assets · 61.2k tokens · 8 ms

 QUALITY GATE   FAILED
   ✖ severity ≥ high        3 issues   (allowed 0)
   ✔ context debt           6.1%       (max 10%)

   Security      E   2 vulnerabilities
   Reliability   B   1 bug
   Clarity       C   5 smells
   Efficiency    B   context debt 6.1%
   Context load      18.4k always-on tokens · 9.2% of 200k window

   CRITICAL 2   HIGH 1   MEDIUM 4   LOW 7   INFO 0

 CLAUDE.md
   12:5    CRITICAL  AI-SEC-001  Invisible character U+202E RIGHT-TO-LEFT OVERRIDE
                     ↳ Remove the character.
   40:1    MEDIUM    AI-COG-001  Contradicts line 12: "Always use TypeScript" vs "Use plain JavaScript"

 .mcp.json
   7:16    HIGH      AI-SEC-005  Unpinned MCP package "@modelcontextprotocol/server-github" (npx -y)
```

- Colors: gate PASSED green / FAILED red background; ratings A green, B lime, C yellow, D orange, E red; severities colored. No color when not a TTY, with `--color never`, or when `NO_COLOR` is set.
- The remediation line (`↳`) is shown for Critical and High only (keeps output short), or for all with `-v`.
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
  "metrics": { "assets": 42, "total_tokens": 61230, "always_on_tokens": 18400, "context_load": 0.092, "context_debt": 0.061,
               "issues": { "critical": 2, "high": 1, "medium": 4, "low": 7, "info": 0 } },
  "findings": [ { "rule": "AI-SEC-001", "severity": "critical", "category": "security", "path": "CLAUDE.md",
                  "line": 12, "col": 5, "end_line": 12, "end_col": 6, "message": "…", "remediation": "…",
                  "token_waste": 0, "fingerprint": "9f86d081884c7d65", "new": true } ]
}
```

Gate values: severities and ratings are strings, ratios and token counts numbers, "no open finding" is `null`. `metrics.issues` and the gate count every open finding; `--min-severity` only filters `findings`. `category` is `null` for custom rules; `fingerprint` is 16 hex digits; `new` is `true` for every finding until baselines exist (P6.3). Object keys keep the order above.

## SARIF 2.1.0 (`--format sarif`)

For GitHub Code Scanning. `tool.driver.rules[]` comes from `RuleMeta` (id, name, shortDescription, help.markdown from the embedded docs, `properties.tags = [category]`, `defaultConfiguration.level`). Mapping: Critical/High → `error`, Medium → `warning`, Low/Info → `note`. Also emit `properties.security-severity` (Critical 9.5, High 8.0, Medium 5.5, Low 3.0) for security rules. `partialFingerprints.pulse/v1` = fingerprint. `rules[]` lists only the rules that have results, in ID order (`ruleIndex` points into it); `help.text` is the remediation; custom rules carry only their `id`. Results use `artifactLocation.uri` relative to the root (percent-encoded, `uriBaseId: "%SRCROOT%"`) and `columnKind: "unicodeCodePoints"`; object keys are sorted. Validated in tests against the vendored schema `crates/pulse-report/tests/schemas/sarif-schema-2.1.0.json` (ADR-034).

## Markdown (`--format markdown`)

For PR comments: a gate badge line, a ratings table, an issue-count table, then findings grouped by file in a collapsible `<details>` per file. Capped at 50 findings, with a "+N more" line.

## GitHub (`--format github`)

GitHub Actions workflow commands, one per finding: `::error file=CLAUDE.md,line=12,col=5,title=AI-SEC-001::Invisible character…`. Critical/High → `error`, Medium → `warning`, Low/Info → `notice`. Then a plain-text gate summary. If `GITHUB_STEP_SUMMARY` is set, the Markdown report is also appended to that file.
