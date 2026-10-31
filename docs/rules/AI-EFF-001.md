# AI-EFF-001 — Asset over token budget

**Severity:** High (Medium when ≤ 125% of budget) · **Category:** Efficiency · **Applies to:** instructions, scoped instructions, agents, prompts, skills, tools, custom assets · **Inline-suppressible:** yes

## Why it matters
Every token in an AI asset costs context window, money and latency each time the file is loaded. Oversized files also dilute the instructions that matter: models follow long, sprawling prompts less reliably. A budget per kind keeps each file focused.

## What triggers it
- An asset's estimated tokens exceed the `[budgets]` entry for its kind. Defaults: instructions 3000, scoped_instructions 1500, agent 2500, prompt 2000, skill 5000, tools 3000, custom 3000.
- A Kiro JSON agent (`.kiro/agents/*.json`) counts its inline `prompt` text only.
- For skills only the body counts; the frontmatter `description` is always-on context (AI-EFF-002).
- Up to 125% of the budget the finding is Medium; beyond that it is High. The excess tokens are reported as token waste.
- MCP configs, settings and hooks have no token budget.

## Noncompliant
```markdown
# CLAUDE.md
(4,000 tokens of style guide, API reference and changelog copied inline)
```

## Compliant
```markdown
# CLAUDE.md
Core conventions in a few hundred tokens. API reference lives in a skill loaded on demand.
```

## Options
None. Budgets are set in `[budgets]` in `pulse.toml`.

## Suppressing
`<!-- pulse-ignore AI-EFF-001 -- reason -->` (Markdown) or a `[[ignore]]` entry in `pulse.toml`.
