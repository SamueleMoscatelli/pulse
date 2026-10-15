# 04 — Scoring & Quality Gate

## Issue types (Sonar mapping)

| Category | Sonar type | Rating it drives |
|---|---|---|
| SEC | Vulnerability | Security |
| STR | Bug | Reliability |
| COG | Code smell (clarity) | Clarity |
| EFF | Code smell (context) | Efficiency |

## Ratings (A–E)

Security, Reliability and Clarity use the worst open severity in their category:

| Worst open severity | Rating |
|---|---|
| none / Info | A |
| Low | B |
| Medium | C |
| High | D |
| Critical | E |

Efficiency uses the **Context debt ratio**:

| Debt ratio | Rating |
|---|---|
| ≤ 5% | A |
| ≤ 10% | B |
| ≤ 20% | C |
| ≤ 50% | D |
| > 50% | E |

## Metrics

- **total_tokens**: Σ `tokens()` over all scanned assets.
- **always_on_tokens**: Σ Always tokens (see `02-assets.md` → LoadMode, Skill/Agent description rule).
- **context_debt** = `min(1.0, Σ token_waste of open EFF findings / max(total_tokens, 1))`. Overlapping waste on the same span counts once (take the max).
- **context_load** = `always_on_tokens / context.window` (default window 200,000). Reported as a percentage. It is a metric only, not a rating; AI-EFF-002 handles the budget.
- **issues_by_severity**, **issues_by_category**, **assets_by_kind**.

Metrics cover the in-scope assets (after `paths` narrowing). Findings of custom (`X-`) rules have no category: they count in `issues_by_severity` and the severity gate condition, not in ratings or `issues_by_category`. Overlap = same file and intersecting half-open ranges (line, col)..(end_line, end_col); identical start points always overlap.

Open findings = findings after suppressions. When `--new-only`/baseline is active, ratings are computed on new findings only and the overall numbers are shown alongside them. (Baseline-aware ratings land with P6.3; until then ratings use all open findings.)

## Quality Gate

The gate is configured in `[gate]`. Default ("Pulse way"):

```toml
[gate]
fail_on_severity = "high"     # any open finding >= this severity fails the gate ("none" disables)
max_context_debt = 0.10       # fail if context_debt > 10%
max_always_on_tokens = 0      # 0 = disabled; otherwise fail if always_on_tokens exceeds it
min_ratings = {}              # e.g. { security = "A", reliability = "B" }
new_code_only = false         # evaluate conditions on new findings only (needs a baseline or --changed-since)
```

Evaluation produces `Gate { status: Passed | Failed, conditions: Vec<Condition { name, actual, threshold, passed }> }`. Every enabled condition is listed in the report, passed or failed; disabled ones (`fail_on_severity = "none"`, `max_always_on_tokens = 0`, unset `min_ratings` keys) are omitted. `max_context_debt` is always enabled. `fail_on_severity` passes when the worst open severity is below the threshold; a rating passes when it is equal to or better than its minimum. `new_code_only` is applied by P6.3 (baseline).

## Fingerprints (baseline / new code)

`fingerprint = xxh3(rule_id, normalized relative path, trimmed text of the finding's first line, occurrence index of that (rule, path, line-text) triple)`. Fingerprints are stable when lines move. The baseline file `.pulse/baseline.json` stores `{ version: 1, fingerprints: [hex…] }`, sorted, so diffs stay clean.
