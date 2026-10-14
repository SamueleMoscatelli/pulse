# AI-EFF-002 — Always-on context over budget

**Severity:** High · **Category:** Efficiency · **Applies to:** the whole project · **Inline-suppressible:** yes

## Why it matters
Some context is loaded into every session before you type anything: root instructions, always-applied rules, every skill and agent description, tool schemas. Each file may be small, but together they can eat a large slice of the context window on every request. This rule budgets the total.

## What triggers it
- The sum of always-on tokens exceeds `budgets.always_on` (default 8000). It counts always-loaded instructions and rules (including `alwaysApply: true` / `applyTo: "**"` scoped rules), tool definitions, and only the `name`/`description` of skills and agents.
- The finding points at the largest contributor and lists the top 5 contributors with their tokens. The excess is reported as token waste.
- `@imports` in root instructions are not counted yet.

## Noncompliant
```text
CLAUDE.md 6200 · .cursor/rules/style.mdc (alwaysApply) 1800 · 40 skill descriptions 900  →  8900 > 8000
```

## Compliant
```text
CLAUDE.md 2500 · style.mdc scoped with globs · skill descriptions 900  →  3400 ≤ 8000
```

## Options
None. The budget is `budgets.always_on` in `pulse.toml`.

## Suppressing
`<!-- pulse-ignore AI-EFF-002 -- reason -->` in the largest contributor (Markdown) or a `[[ignore]]` entry in `pulse.toml`.
