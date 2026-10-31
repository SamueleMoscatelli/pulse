# AI-STR-005 — Unknown tool in allow-list

**Severity:** Medium · **Category:** Structure · **Applies to:** agents (Kiro JSON agents included), prompts/commands, settings · **Inline-suppressible:** yes

## Why it matters
Tool names in allow-lists are matched exactly. A typo (`Reed`, `WebFecth`) or an MCP tool whose server is not configured grants nothing: the agent lacks a tool you meant to give it, or a deny rule you rely on never applies.

## What triggers it
- **Claude Code** (settings `permissions.allow/deny/ask`, agent `tools`, command `allowed-tools`): an entry whose tool (the part before `(`) is not a built-in tool. The message suggests the closest built-in.
- An `mcp__<server>__<tool>` entry whose `<server>` is not defined in any MCP config of the project (`.mcp.json`, `.vscode/mcp.json`, …). Not checked when the project has no MCP config at all (servers may be configured per user), nor for servers Claude Code provides itself (`ide`, `claude_ai_*`, `plugin_*`).
- **Kiro** (`.kiro/agents/*.json` and `*.md`, `tools` and `allowedTools`): a name that is not a [Kiro built-in tool](https://kiro.dev/docs/tools/), category tag (`read`, `write`, `shell`, `web`, `subagent`), `*`, or CLI 2.x name still found in agents (`fs_read`, `fs_write`, `execute_bash`, `execute_cmd`, `use_aws`, `todo_list`, …). Selectors are not checked: `@builtin`, `@mcp`, `@server`, `@server/tool`, `subagent/<agent>` and wildcards (MCP servers may be configured per user).
- **GitHub Copilot** (`.github/agents`, `.github/prompts`, `.github/chatmodes`, and any `*.agent.md`/`*.prompt.md` `tools`): only entries one edit away from a built-in tool (`codebas` → `codebase`), since extensions contribute other tools. Names are compared case-insensitively; tool sets (`server/*`, `edit/editFiles`), MCP server names and the Copilot aliases (`Read`, `Edit`, `shell`, `Bash`, `Task`, …) are accepted.

## Noncompliant
```yaml
---
name: issue-filer
tools: Reed, mcp__jira__create_ticket
---
```

## Compliant
```yaml
---
name: issue-filer
tools: Read, mcp__github__create_issue   # `github` is defined in .mcp.json
---
```

## Options
None.

## Suppressing
`<!-- pulse-ignore AI-STR-005 -- reason -->` (Markdown) or a `[[ignore]]` entry in `pulse.toml`.
