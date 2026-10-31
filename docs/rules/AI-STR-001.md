# AI-STR-001 — Invalid or missing frontmatter

**Severity:** High (unknown keys: Low) · **Category:** Structure · **Applies to:** skills, agents, prompts, scoped instructions (including Kiro steering) · **Inline-suppressible:** yes

## Why it matters
Agents read an asset's frontmatter to decide whether and when to load it. A skill without a `description` is never picked, a Cursor rule without `globs`/`alwaysApply`/`description` never applies, and a misspelled key is silently ignored, so the asset quietly does nothing.

## What triggers it
- A file that opens with `---` but never closes the block (the whole file is then read as body).
- A missing required key, per platform (taken from the path):
  - Claude skill (`SKILL.md`) and Claude agent (`.claude/agents/`): `name` and `description`.
  - Cursor rule (`.mdc`): at least one of `description`, `globs`, `alwaysApply`.
  - Copilot instructions (`.instructions.md`): `applyTo`.
  - Kiro steering (`.kiro/steering/`): `inclusion` must be `always`, `fileMatch`, `manual` or `auto` (with a "did you mean" hint); `fileMatch` needs `fileMatchPattern` (a glob or a list of globs); `auto` needs `name` and `description`. Known keys: `inclusion`, `fileMatchPattern`, `name`, `description`, `title`.
- Kiro steering whose frontmatter is not on the first line (a comment or blank line above the `---` block): Kiro ignores it and always includes the file.
- A required key with a null value counts as missing.
- An unknown key (Low), with a "did you mean" hint when a known key is within 2 edits.

Claude commands, Copilot prompts (`.prompt.md`) and Copilot agents (`.agent.md`, `.chatmode.md`) have no required keys but are checked for unknown ones. `.prompty`, `.claude/rules`, Windsurf and Cline files are not checked. Invalid YAML is reported by AI-STR-002.

## Noncompliant
```markdown
---
name: pdf-tools
descripton: Extract text from PDF files.
---
```

## Compliant
```markdown
---
name: pdf-tools
description: Extract text from PDF files. Use when the user mentions PDFs.
---
```

## Options
None.

## Suppressing
`<!-- pulse-ignore AI-STR-001 -- reason -->` (Markdown) or a `[[ignore]]` entry in `pulse.toml`.
