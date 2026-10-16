# AI-STR-004 — Name / description constraint violated

**Severity:** Medium · **Category:** Structure · **Applies to:** skills, agents, prompts · **Inline-suppressible:** yes

## Why it matters
Platforms validate skill metadata when loading it: a skill whose name has the wrong format, uses a reserved word, or doesn't match its folder can be rejected or shadowed, and an empty or oversized description gives the model nothing useful to decide when to use the asset.

## What triggers it
- A skill `name` that is not lowercase letters and digits separated by single hyphens (`^[a-z0-9]+(-[a-z0-9]+)*$`).
- A skill `name` longer than 64 characters.
- A skill `name` containing `anthropic` or `claude`.
- A skill `name` different from the directory holding its `SKILL.md`.
- A `description` (skills, agents, prompts) that is empty or longer than 1024 characters.

## Noncompliant
```markdown
<!-- .claude/skills/pdf/SKILL.md -->
---
name: Claude_PDF
description: ""
---
```

## Compliant
```markdown
<!-- .claude/skills/pdf-tools/SKILL.md -->
---
name: pdf-tools
description: Extract text and tables from PDF files. Use when the user mentions PDFs.
---
```

## Options
None.

## Suppressing
`<!-- pulse-ignore AI-STR-004 -- reason -->` (Markdown) or a `[[ignore]]` entry in `pulse.toml`.
