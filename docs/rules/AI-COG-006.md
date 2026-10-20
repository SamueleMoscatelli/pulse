# AI-COG-006 — Weak routing description

**Severity:** Medium · **Category:** Cognitive · **Applies to:** skills, agents · **Inline-suppressible:** yes

## Why it matters
The model decides whether to load a skill or delegate to an agent from its `description` alone. A description that is too short, never says when to use it, or speaks as "I" leads to missed or wrong invocations.

## What triggers it
One finding at the `description` line when any of these holds (the message lists which):
- Fewer than 20 words.
- No trigger cue: `use`/`used` followed within 4 words by `when`, `whenever`, `for` or `if` ("Use this agent when …"), `when the user`, or a word starting with `trigger`, `invok` or `proactive`.
- First person: `I`, `I'm`, `I'll`, `I've`, `I'd` outside quotes or code spans (quoted user speech such as `"I need a review"` is fine).

A missing or empty description is AI-STR-001's, not this rule's.

## Noncompliant
```markdown
---
name: pdf-tools
description: I extract text from PDF files.
---
```

## Compliant
```markdown
---
name: pdf-tools
description: Extract text, tables and form fields from PDF files and merge or split documents. Use when the user asks to read, fill or combine PDFs.
---
```

## Options
None.

## Suppressing
`<!-- pulse-ignore AI-COG-006 -- reason -->` (Markdown) or a `[[ignore]]` entry in `pulse.toml`.
