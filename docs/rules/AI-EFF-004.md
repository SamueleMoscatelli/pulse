# AI-EFF-004 — Inlined bulk content

**Severity:** Medium · **Category:** Efficiency · **Applies to:** instructions, scoped instructions, prompts (when always or conditionally loaded) · **Inline-suppressible:** yes

## Why it matters
Large code samples, reference tables and config dumps are rarely needed on every turn, yet when they sit inside an always-loaded file they cost tokens in every session. Moved to a separate file and referenced, they are read only when the task needs them.

## What triggers it
- A fenced or indented code block with more than 40 lines of content (fences not counted).
- A JSON or YAML block (`json`, `jsonc`, `json5`, `yaml`, `yml`) with more than 60 lines of content; data blocks get this higher limit instead of 40.
- A Markdown table with more than 25 body rows.
- Only assets loaded always or conditionally are checked: on-demand prompts and skill/agent bodies are not.
- Token waste: the block's tokens minus 20 (the cost of the reference line that replaces it).

## Noncompliant
````markdown
# CLAUDE.md
```json
{ "...": "120 lines of example config" }
```
````

## Compliant
```markdown
# CLAUDE.md
Example config: see [docs/config-example.json](docs/config-example.json).
```

## Options
None.

## Suppressing
`<!-- pulse-ignore AI-EFF-004 -- reason -->` (Markdown) or a `[[ignore]]` entry in `pulse.toml`.
