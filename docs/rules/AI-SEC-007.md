# AI-SEC-007 — Over-permissive agent permissions

**Severity:** High (Medium for some checks) · **Category:** Security · **Applies to:** settings, agents (Kiro JSON agents included), prompts/commands, MCP configs, and bypass flags in any asset · **Inline-suppressible:** yes

## Why it matters
An agent can only do what its permissions allow, and prompt injection turns an agent's permissions into the attacker's. Allowing every shell command, every URL, or every tool call without a prompt means one malicious file or web page is enough to run code or send your source and secrets elsewhere. Shell plus network access together is the classic exfiltration chain.

## What triggers it
- Settings (`.claude/settings.json`): `permissions.allow` containing `Bash`, `Bash(*)`, `Bash(:*)` or `*`; `WebFetch` (or `WebFetch(*)`) without a domain; `defaultMode: "bypassPermissions"`; `enableAllProjectMcpServers: true` (Medium).
- Agent and prompt/command frontmatter: `tools` or `allowed-tools` with unrestricted `Bash` (Medium), or High when the same list also has `WebFetch` or `WebSearch`.
- Kiro agents (`.kiro/agents/*.json` and `*.md`):
  - `allowedTools` pre-approving the shell (`shell`, `execute_bash`, `execute_cmd`): Medium; High with a web tool (`web`, `web_fetch`, `web_search`, `remote_web_search`). `*` and `@builtin` pre-approve every tool: High. Not reported when `toolsSettings` restricts the shell tool (`allowedCommands`).
  - `tools: ["*"]` with no `permissions` (Medium).
  - A `permissions` rule that allows every shell command: `effect: allow`, no `exclude`, and no `match` or a `*`/`**` one, for capability `shell` (Medium) or `all`/`builtin` (High).
  - Write access with no path limit: `allowedTools` with `fs_write`, `write`, `fs_append`, `str_replace`, `*` or `@builtin` (unless `toolsSettings` restricts `fs_write`), or an allow rule for `fs_write`/`filesystem`/`all`/`builtin` matching everything, `~/**` or `~/.kiro/**` (Medium). Kiro warns that such an agent can edit `~/.kiro`: other agents, steering, skills and MCP configs.
  - A JSON agent's inline `mcpServers` get the MCP config checks below.
- Any asset: the text `--dangerously-skip-permissions` or `--yolo` (as a whole flag).
- MCP configs (`.mcp.json`, `.kiro/settings/mcp.json`, …): `alwaysAllow` / `autoApprove` lists containing `*` or a tool whose name has a write/exec word (`write_file`, `executeCommand`, `git_push`, …). Disabled servers are checked too: the list applies as soon as someone turns the server on. Hiding tools with Kiro `disabledTools` doesn't change the result.

Entries under `permissions.deny` and `permissions.ask` are not checked. Scoped entries such as `Bash(git diff:*)` and `WebFetch(domain:docs.rs)` are fine.

## Noncompliant
```json
{ "permissions": { "allow": ["Bash(*)", "WebFetch"], "defaultMode": "bypassPermissions" } }
```

## Compliant
```json
{ "permissions": { "allow": ["Bash(npm run test:*)", "WebFetch(domain:docs.rs)"], "ask": ["Bash"] } }
```

A Kiro agent (`.kiro/agents/builder.json`) that allows specific commands and paths:
```json
{
  "tools": ["read", "write", "shell"],
  "permissions": { "rules": [
    { "capability": "shell", "match": ["cargo *", "git *"], "effect": "allow" },
    { "capability": "fs_write", "match": ["src/**", "tests/**"], "effect": "allow" }
  ] }
}
```

## Options
None.

## Suppressing
`<!-- pulse-ignore AI-SEC-007 -- reason -->` (Markdown) or a `[[ignore]]` entry in `pulse.toml`.
