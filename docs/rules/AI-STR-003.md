# AI-STR-003 — Broken reference or import

**Severity:** Medium (import cycles: High) · **Category:** Structure · **Applies to:** Markdown assets · **Inline-suppressible:** yes

## Why it matters
An `@import` (or Kiro `#[[file:…]]`) of a missing file is silently dropped: the instructions you think are loaded never reach the model. A broken link or a skill pointing at a missing `scripts/…` file sends the agent looking for something that isn't there. Import cycles make the loaded context depend on hop limits and waste tokens.

## What triggers it
- An `@path` import (`@docs/style.md`, `@/shared/rules.md`) whose file does not exist, relative to the asset (or to the scan root for `@/`).
- A Kiro reference whose file or folder does not exist: `#[[file:<path>]]`, `#[[file:<path>:<line>]]`, `#[[file:<path>:<start>-<end>]]`, `#[[folder:<path>]]`. In `.kiro/steering/` files paths resolve from the workspace root (the folder holding `.kiro/`); in `AGENTS.md`, from its folder. A line range that starts at 0, starts after it ends, or ends past the last line is reported too. Other files don't expand these references, so they are not checked there.
- A Kiro agent `file://` URI to a missing file: `prompt` resolves from the agent's folder; `resources` from the project root (the folder holding `.kiro/`) or the agent's folder. Globs (`file://docs/**/*.md`), absolute and `~` paths are not checked. In a JSON agent this is the only check (its `prompt` is not Markdown).
- A relative Markdown link or image (`[guide](docs/guide.md)`) whose file or directory does not exist. `#anchors` and `?queries` are ignored.
- In skills, a `scripts/…`, `references/…` or `assets/…` path that exists neither relative to the skill folder nor to the scan root. A path shown only as code (`` `assets/preview.png` `` or inside a code block) is reported only when it is run by a command (`python scripts/x.py`, `uv run scripts/x.py`, `bash scripts/x.sh`, …) or its first folder exists (a likely typo); otherwise it is a file the skill asks for or creates.
- An import that is part of a cycle of at most 5 hops (`a.md` → `b.md` → `a.md`): **High**, reported on each import of the cycle.

Not reported: URLs, anchor-only links, `@mentions` without `/` or `.` (`@alice`), home paths (`@~/…`), paths outside the scan root, and placeholder paths containing `<`, `your-` or `example`. Files excluded from the scan still count as existing.

## Noncompliant
```markdown
# Setup
Follow @docs/setup.md   <!-- docs/setup.md was renamed -->
```

## Compliant
```markdown
# Setup
Follow @docs/getting-started.md
```

## Options
None.

## Suppressing
`<!-- pulse-ignore AI-STR-003 -- reason -->` or a `[[ignore]]` entry in `pulse.toml`.
