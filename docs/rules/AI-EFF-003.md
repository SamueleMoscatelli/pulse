# AI-EFF-003 — Duplicated content

**Severity:** Medium · **Category:** Efficiency · **Applies to:** Instructions, scoped instructions, agents, prompts, skills · **Inline-suppressible:** yes

## Why it matters
The same guidance in two places costs tokens twice, every session it is loaded in, and the copies drift apart: one gets updated, the other doesn't, and the model sees two versions of a rule.

## What triggers it
- **Paragraphs (≥ 25 words)** repeated or lightly reworded, within one file or across files: at least half of their 5-word shingles in common (estimated with MinHash). Reported on the later occurrence in path order, naming the earlier one; "Duplicate" when the wording matches, "Near-duplicate" otherwise.
- **Lines (≥ 8 words)** repeated word for word in another always-on asset (always-loaded instructions and prompts). Lines in fenced code, the frontmatter, or a paragraph already reported are skipped.

Words are compared after normalization (case, Unicode lookalikes, Markdown emphasis and punctuation ignored).

Not reported: copies in contexts never loaded together. A file's context is the folder holding it, or the folder above its platform directory (`pkg/.claude/…` → `pkg`) or its `skills/` folder; two files are compared only when one context contains the other, so `packages/a/CLAUDE.md` and `packages/b/CLAUDE.md` don't match, while the root `CLAUDE.md` matches both.

`token_waste` is the estimated tokens of the reported copy.

## Noncompliant
```text
CLAUDE.md   Before opening a pull request, run the full test suite locally, update the
            changelog with every user-facing change, and ask a maintainer to review …
AGENTS.md   Before you open a pull request, run the complete test suite locally, update
            the changelog with every user-facing change, and ask a maintainer to review …
                                                                       ← AI-EFF-003
```

## Compliant
```text
AGENTS.md   Before opening a pull request, run the full test suite locally, …
CLAUDE.md   @AGENTS.md
```

## Options
None.

## Suppressing
`<!-- pulse-ignore AI-EFF-003 -- reason -->` or a `[[ignore]]` entry in `pulse.toml`.
