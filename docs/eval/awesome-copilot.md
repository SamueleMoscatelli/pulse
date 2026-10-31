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

## v1.1.0 candidate

`scripts/compare.sh` with v1.0.0 (old) vs `main` after P8.8 (new), on 2026-10-31. Corpus at
`0fe6545b`, default config, `--min-severity info`. The new binary still reports `1.0.0`: the
version bump is part of the release PR.

### Totals

| metric | v1.0.0 | candidate | Δ |
|---|---:|---:|---:|
| assets | 476 | 908 | +432 |
| findings | 982 | 1392 | +410 |
| critical | 10 | 20 | +10 |
| high | 36 | 204 | +168 |
| medium | 408 | 745 | +337 |
| low | 26 | 97 | +71 |
| info | 502 | 326 | -176 |
| gate | failed | failed | 0 |
| context load | 0.2133 | 0.2133 | 0 |
| always-on tokens | 42653 | 42653 | 0 |

The asset count nearly doubles because library repos are now scanned (P8.1–P8.3): the
`agents/`, `instructions/` and `prompts/` trees of the corpus were invisible to v1.0.0. Of the
added findings, only 29 sit in files that v1.0.0 already reported on (15 AI-STR-008, 11
AI-EFF-003, 3 AI-COG-003); the rest are new coverage. Always-on load is unchanged: library
assets are on-demand.

### Per rule

| rule | v1.0.0 | candidate | Δ | why |
|---|---:|---:|---:|---|
| AI-COG-003 | 379 | 23 | -356 | weak prohibitions only (P8.4) |
| AI-COG-006 | 214 | 423 | +209 | library agents/skills now scanned |
| AI-EFF-001 | 33 | 221 | +188 | library instructions/agents now scanned |
| AI-COG-002 | 105 | 287 | +182 | coverage |
| AI-EFF-003 | 45 | 76 | +31 | coverage (cross-file duplicates between library instructions) |
| AI-SEC-013 | 29 | 57 | +28 | coverage |
| AI-EFF-007 | 15 | 40 | +25 | coverage |
| AI-STR-001 | 2 | 26 | +24 | coverage; Copilot agent schema fixed in P8.8 (64 before) |
| AI-COG-004 | 10 | 30 | +20 | coverage |
| AI-STR-008 | 0 | 20 | +20 | new rule (P8.3) |
| AI-STR-003 | 23 | 36 | +13 | coverage |
| AI-SEC-002 | 5 | 12 | +7 | coverage |
| AI-SEC-010 | 8 | 12 | +4 | coverage |
| AI-COG-007 | 7 | 10 | +3 | coverage |
| AI-SEC-003 | 0 | 3 | +3 | coverage; URL/path guard added in P8.8 (6 before) |
| AI-SEC-001 | 1 | 3 | +2 | coverage |
| AI-SEC-007 | 0 | 2 | +2 | coverage |
| AI-COG-001, AI-SEC-005, AI-SEC-011, AI-STR-002, AI-STR-007 | 11 | 16 | +5 | coverage, +1 each |
| AI-EFF-006, AI-SEC-008, AI-STR-004 | 95 | 95 | 0 | |

Other repos: this repo 9 → 2 (-7 AI-COG-003); `tests/fixtures/repos/advice` 3 → 2 (-1
AI-COG-003); `clean`, `mixed`, `ondemand`, `single` unchanged. `library` can't be compared:
v1.0.0 rejects its `[library]` config table (`unknown field`), as expected for a new key; the
candidate reports 9 findings there (AI-STR-008, AI-COG-006), all intended by the fixture.

### Sampled verdicts

20 random added and 20 random removed findings (Python `random.seed(88)` over the
fingerprint diff, drawn before the P8.8 fixes; none fell in the fixed clusters).

**Added: 20/20 correct.** 6 AI-COG-002 (`etc`, `best practice`, `as needed`, `appropriate(ly)`
×3), 4 AI-EFF-001 (5.9k, 5.2k, 7.4k and 1.5k-token library files), 3 AI-COG-006 (11–17-word
descriptions with no trigger cue), 3 AI-EFF-003 (one is a repeated 5-line template block
inside one agent; two are Bicep/Terraform and Clojure instruction pairs), 2 AI-STR-001
(`mode` on an agent: correct, but the hint "did you mean model" is misleading, see backlog),
1 AI-COG-004 (48 of 82 directives emphasized), 1 AI-COG-003 ("Do NOT insist the user is
wrong.").

**Removed: 20/20 correct.** All are AI-COG-003 "negative-only constraint" findings, which
P8.4 narrowed to weak prohibitions. 4 of them were misparses in v1.0.0 ("Optionally
demonstrate file upload…", "`No profile found`", "Clearly list what is _not_ included…",
"Only document what…"); the rest were specific prohibitions ("Never skip validation", "No
system-level installs.") that need no positive counterpart.

### Regressions

Fixed here:
- **AI-STR-001** flagged 38 valid Copilot agent keys (`user-invocable` ×22,
  `disable-model-invocation` ×13, `agents` ×3). The agent schema now has every field in the
  VS Code custom agent file reference (`agents`, `user-invocable`, `disable-model-invocation`,
  `skills`, `hooks`, `model-policy`, `reasoning-effort`, …).
- **AI-SEC-003** flagged a URL (`"token_url": "https://…"`) and a word path
  (`Figma Token: "Desktop/Title/H1"`) as generic secrets. Values with `://`, or made only of
  `/`-separated words, now count as code; base64 with slashes still matches.

Left (backlog): the `mode` hint on Copilot agents; v1.0.0 hard-fails on config keys added
later.

Remaining AI-SEC-003 hits are deliberate bad-practice examples (`API_KEY=actual_secret_key_12345`,
`sk_live_…` in "don't do this" code); the rule flags them by design and suppression is the
fix.

### Approval

Approved by the user on 2026-10-31 after testing the installed build. Changes made during the
review: terminal lines wrap at the window width (ADR-047); the CONTEXT LOAD block shows one
labeled bar per assistant and explains the budget; `--assistant` scans for selected assistants
only (ADR-048). Default scans of the corpus are unchanged by these (compare: 0 delta).
