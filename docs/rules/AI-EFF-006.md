# AI-EFF-006 — Oversized skill or agent

**Severity:** Medium · **Category:** Efficiency · **Applies to:** skills, agents · **Inline-suppressible:** yes

## Why it matters
A skill or agent `description` is loaded into every session so the model can decide when to use it; a long description is paid for on every turn, used or not. A very long skill body is expensive each time the skill runs and is better split into files the skill reads as needed.

## What triggers it
- A skill body (after the frontmatter) longer than 500 lines. The finding points at line 501; token waste is the tokens past it.
- A skill or agent `description` over `budgets.description` tokens (default 150). Token waste is the excess.

## Noncompliant
```markdown
---
name: reviewer
description: Use this agent to review code. It checks style, naming, tests, security, performance, docs… (200 more words)
---
```

## Compliant
```markdown
---
name: reviewer
description: Reviews a diff for bugs and convention violations. Use after finishing a change.
---
```

## Options
None. The description budget is `budgets.description` in `pulse.toml`.

## Suppressing
`<!-- pulse-ignore AI-EFF-006 -- reason -->` (Markdown) or a `[[ignore]]` entry in `pulse.toml`.
