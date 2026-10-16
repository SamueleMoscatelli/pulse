# AI-STR-007 — Duplicate asset name

**Severity:** High · **Category:** Structure · **Applies to:** Skills, agents, prompts · **Inline-suppressible:** yes

## Why it matters
When two skills, agents or commands resolve to the same invocation name, the platform loads only one of them (or picks one unpredictably). The other silently never runs, and which one wins can change with the load order.

## What triggers it
Two assets of the same kind and platform with the same invocation name. Reported on the second one in path order, naming the first.

- **Skills and agents:** the frontmatter `name`; without one, the skill folder name or the agent file stem (`planner.agent.md` → `planner`).
- **Claude commands:** the path under `.claude/commands/`, with `/` as `:` (`.claude/commands/git/commit.md` → `/git:commit`).
- **Copilot prompts:** the frontmatter `name`, else the file stem (`review.prompt.md` → `/review`).

The platform is the config directory in the path (`.claude`, `.github`, `.cursor`, `.agents`, `.codex`, `.gemini`, `.windsurf`), wherever it sits in the tree. Outside those, names are unique per parent folder of the asset folder (`plugins/a/skills`), so separate plugins don't collide.

Not reported: the same name on different platforms or in different plugins, a skill and an agent sharing a name, and `.prompty` files (they have no invocation name).

## Noncompliant
```text
.claude/skills/code-review/SKILL.md   name: review
.claude/skills/review/SKILL.md        name: review   ← AI-STR-007
```

## Compliant
```text
.claude/skills/review/SKILL.md        name: review
.github/skills/review/SKILL.md        name: review   (different platform)
```

## Options
None.

## Suppressing
`<!-- pulse-ignore AI-STR-007 -- reason -->` or a `[[ignore]]` entry in `pulse.toml`.
