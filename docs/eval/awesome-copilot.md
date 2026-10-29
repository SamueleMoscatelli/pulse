# Corpus baseline: github/awesome-copilot

The Phase 8 accuracy work is measured against this baseline: pulse **v1.0.0** on
[github/awesome-copilot](https://github.com/github/awesome-copilot) at commit `0fe6545b`
(2026-10-28), default config, every severity (`--min-severity info`).

Reproduce (clone next to this repo):

```sh
git clone https://github.com/github/awesome-copilot ../awesome-copilot
git -C ../awesome-copilot checkout 0fe6545b
scripts/compare.sh ../awesome-copilot   # v1.0.0 on PATH → target/release/pulse
```

Each Phase 8 PR reports its delta from `scripts/compare.sh`. The full added/removed lists go to
`target/compare/awesome-copilot.md`.

## Totals

| metric | v1.0.0 |
|---|---:|
| assets | 476 |
| findings | 982 |
| critical | 10 |
| high | 36 |
| medium | 408 |
| low | 26 |
| info | 502 |
| gate | failed (fail_on_severity, max_context_debt) |
| context load | 0.2133 |
| context debt | 0.2189 |
| always-on tokens | 42653 |
| total tokens | 1112404 |

## Findings per rule

| rule | findings |
|---|---:|
| AI-COG-001 | 1 |
| AI-COG-002 | 105 |
| AI-COG-003 | 379 |
| AI-COG-004 | 10 |
| AI-COG-006 | 214 |
| AI-COG-007 | 7 |
| AI-EFF-001 | 33 |
| AI-EFF-003 | 45 |
| AI-EFF-006 | 76 |
| AI-EFF-007 | 15 |
| AI-SEC-001 | 1 |
| AI-SEC-002 | 5 |
| AI-SEC-008 | 4 |
| AI-SEC-010 | 8 |
| AI-SEC-011 | 10 |
| AI-SEC-013 | 29 |
| AI-STR-001 | 2 |
| AI-STR-003 | 23 |
| AI-STR-004 | 15 |
