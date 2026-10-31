# Corpus baseline: Kiro

The Phase 9 Kiro work is measured against this baseline: pulse **v1.1.0** on two real Kiro
repos, default config, every severity (`--min-severity info`), on 2026-10-31.

| repo | commit | license | shape |
|---|---|---|---|
| [affaan-m/ECC](https://github.com/affaan-m/ECC) | `b38be6a` (2026-10-29) | MIT | multi-assistant repo with a live `.kiro/` |
| [mehtadman87/kiro-config](https://github.com/mehtadman87/kiro-config) | `89f77b9` (2026-04-03) | MIT | a power: `POWER.md`, `steering/`, `hooks/`, `specs/` at the root |

Reproduce (clone next to this repo):

```sh
git clone https://github.com/affaan-m/ECC ../ECC
git -C ../ECC checkout b38be6a
git clone https://github.com/mehtadman87/kiro-config ../kiro-config
git -C ../kiro-config checkout 89f77b9
scripts/compare.sh ../ECC ../kiro-config   # v1.1.0 on PATH → target/release/pulse
```

Each Phase 9 PR reports its delta from `scripts/compare.sh` on both repos. The full
added/removed lists go to `target/compare/<repo>.md`. Baseline check:
`scripts/compare.sh --new "$(command -v pulse)" ../ECC ../kiro-config` with v1.1.0 on PATH
reports 0 delta on both.

## Totals

| metric | ECC | kiro-config |
|---|---:|---:|
| assets | 1102 | 1 |
| findings | 2441 | 5 |
| critical | 25 | 0 |
| high | 64 | 4 |
| medium | 1680 | 1 |
| low | 595 | 0 |
| info | 77 | 0 |
| gate | failed (fail_on_severity, max_context_debt) | failed (fail_on_severity) |
| context load | 0.2999 | 0.0 |
| context debt | 0.1574 | 0.0 |
| always-on tokens | 59990 | 0 |
| total tokens | 2511945 | 294 |

ECC's always-on load is the Copilot one (59990); Claude Code 52431, Codex 6406, Cursor and
Windsurf 3294, Gemini CLI 439. Kiro isn't an assistant in v1.1.0, so none of it is Kiro's.

## Findings per rule

| rule | ECC | of which in `.kiro/` | kiro-config |
|---|---:|---:|---:|
| AI-COG-001 | 2 | | |
| AI-COG-002 | 53 | 3 | |
| AI-COG-003 | 9 | 1 | |
| AI-COG-004 | 1 | | |
| AI-COG-006 | 554 | 25 | |
| AI-COG-007 | 7 | | |
| AI-EFF-001 | 95 | 6 | |
| AI-EFF-002 | 1 | | |
| AI-EFF-003 | 683 | 15 | |
| AI-EFF-004 | 5 | | |
| AI-EFF-006 | 156 | 10 | |
| AI-SEC-002 | 10 | | |
| AI-SEC-003 | 14 | 3 | |
| AI-SEC-005 | | | 4 |
| AI-SEC-007 | 3 | | |
| AI-SEC-010 | 11 | | |
| AI-SEC-011 | 11 | | |
| AI-SEC-012 | 3 | | |
| AI-SEC-013 | 181 | 6 | |
| AI-STR-001 | 582 | 25 | |
| AI-STR-003 | 55 | 11 | |
| AI-STR-004 | 3 | | |
| AI-STR-006 | 1 | | |
| AI-STR-008 | 1 | | 1 |

Every `.kiro/` finding in ECC sits in `.kiro/skills/*/SKILL.md`, the only Kiro files v1.1.0
reads there.

## Assets found by kind

| kind (origin) | ECC | kiro-config |
|---|---:|---:|
| skill (platform) | 1068 | |
| instructions (platform) | 20 | |
| prompt (platform) | 8 | |
| hooks (platform) | 2 | |
| hooks (library row) | 1 | |
| scoped instructions (platform) | 2 | |
| MCP config (platform) | 1 | |
| MCP config (library row) | | 1 |

None of these are attributed to Kiro. The 43 `.kiro/skills/*/SKILL.md` in ECC are matched by
the generic `**/SKILL.md` row and attributed to Copilot and Claude Code. kiro-config's only
asset is `settings/mcp.json`, matched by the library row (ADR-041) and flagged by AI-STR-008
as "not loaded: move it to `.vscode/mcp.json`", which is wrong for a power.

## What v1.1.0 misses

Kiro files in the corpus and what v1.1.0 does with them (✗ = not discovered).

| Kiro file type | ECC | kiro-config | v1.1.0 |
|---|---:|---:|---|
| steering `*.md` (`.kiro/steering/` or a power's `steering/`) | 22 (8 always, 11 fileMatch, 3 manual) | 31 (6 always, 4 auto, 4 fileMatch, 17 manual) | ✗ |
| `#[[file:…]]` references in steering | 0 | 0 | ✗ (none in the corpus: fixtures only) |
| skills `.kiro/skills/*/SKILL.md` | 43 | | found, attributed to Copilot + Claude Code |
| agents `.kiro/agents/*.json` | 33 | | ✗ |
| agents `.kiro/agents/*.md` (frontmatter) | 33 | | ✗ |
| hooks `*.kiro.hook` (legacy JSON) | 13 | 24 | ✗ |
| MCP config `settings/mcp.json` | 1 (`.kiro/settings/mcp.json.example`) | 1 (power root) | ✗ in `.kiro/`; the power copy is a library-row asset with a wrong AI-STR-008 |
| power `POWER.md` | | 1 | ✗ |
| specs `specs/**/*.md` | | 4 | ✗ |

ECC's `.kiro/` also has a README, `docs/` and install scripts; they aren't assets.

Expected direction for later cards: assets and findings up on both repos (steering, agents,
hooks, MCP and the power now scanned), a Kiro always-on load from the `always` steering files,
AI-STR-008 on kiro-config's `settings/mcp.json` gone, and the 43 skills attributed to Kiro.
