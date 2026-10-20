# AI-COG-007 — Unstructured long instructions

**Severity:** Low · **Category:** Cognitive · **Applies to:** instructions, scoped instructions, agents, skills · **Inline-suppressible:** yes

## Why it matters
Headings and short paragraphs let the model (and the people maintaining the file) find the rule that applies. A long flat file or a wall of text buries individual rules, and they get skipped.

## What triggers it
- A body (after the frontmatter) longer than 60 lines with fewer than 2 headings. Reported at the first body line.
- A paragraph of more than 150 words. Reported once per file, at the first such paragraph, with the count and the longest length. Code blocks, lists without blank lines between items, and tables are not paragraphs.

## Noncompliant
```markdown
# Notes
- Run the tests before pushing.
- … 70 more bullets, no sections …
```

## Compliant
```markdown
# Notes
## Testing
- Run the tests before pushing.
## Style
- …
```

## Options
None.

## Suppressing
`<!-- pulse-ignore AI-COG-007 -- reason -->` (Markdown) or a `[[ignore]]` entry in `pulse.toml`.
