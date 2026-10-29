# AI-STR-008 — Asset not loaded from its location

**Severity:** Medium · **Category:** Structure · **Applies to:** Instructions, skills, agents, prompts, hooks, MCP configs · **Inline-suppressible:** no

## Why it matters
Assistants load assets only from fixed folders and exact file names. An agent in `agents/` instead of `.github/agents/`, a `skill.md`, or a `claude.md` that works on your Mac but not on Linux CI is silently ignored: no error, the assistant just behaves as if the file did not exist.

## What triggers it
- **Assets outside any load folder**: `*.agent.md`, `*.chatmode.md`, `*.prompt.md`, `*.instructions.md`, `hooks.json` and `mcp.json` found by suffix only, outside `[scan] library` and outside plugin folders (a folder, or an ancestor, with `plugin.json`, `.claude-plugin/plugin.json` or `.github/plugin/plugin.json`). One finding per folder and kind, e.g. "222 agents in `agents/` are not loaded by any assistant: move them to `.github/agents/`".
- **Skills not in `<skills-dir>/<name>/SKILL.md`**: `.claude/skills/SKILL.md`, `.github/skills/a/b/SKILL.md`, `.claude/skill/x/SKILL.md`. A `SKILL.md` outside any platform or `skills/` folder (a repo that is one skill) is not reported.
- **Near-miss folders**: `.md` files in `.github/agent/`, `.github/instruction/`, `.github/prompt/`, `.claude/agent/`, `.claude/command/`, `.claude/skill/`, and `*.agent.md` directly in `.github/`.
- **Near-miss names**: `skill.md`/`Skill.md`, `copilot-instruction.md`, `copilot-instructions.md` outside `.github/`, `*.mdc` outside `.cursor/rules/`, and `claude.md`/`Claude.md` (loads on case-insensitive file systems only, so not on Linux CI).

The message names the expected location. VS Code can load extra folders (`chat.instructionsFilesLocations`, `chat.agentFilesLocations`, `chat.agentSkillsLocations`), but other assistants and the Copilot coding agent ignore those settings; declare such folders in `[scan] library`.

## Noncompliant
```text
agents/reviewer.agent.md        ← AI-STR-008 (move to .github/agents/)
.claude/skills/SKILL.md         ← AI-STR-008 (expected .claude/skills/<name>/SKILL.md)
.cursor/style.mdc               ← AI-STR-008 (move to .cursor/rules/)
claude.md                       ← AI-STR-008 (rename to CLAUDE.md)
```

## Compliant
```text
.github/agents/reviewer.agent.md
.claude/skills/review/SKILL.md
.cursor/rules/style.mdc
CLAUDE.md
plugins/review/plugin.json + plugins/review/agents/reviewer.agent.md
```

```toml
[scan]
library = ["agents/**", "instructions/**", "skills/**"]   # distribution folders
```

## Options
None.

## Suppressing
A `[[ignore]]` entry in `pulse.toml`, or declare the folder in `[scan] library`. Findings point at files that may not be assets, so inline comments do not apply.
