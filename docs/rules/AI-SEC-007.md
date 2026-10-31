# AI-SEC-007 — Over-permissive agent permissions

**Severity:** High (Medium for some checks) · **Category:** Security · **Applies to:** settings, agents, prompts/commands, MCP configs, and bypass flags in any asset · **Inline-suppressible:** yes

## Why it matters
An agent can only do what its permissions allow, and prompt injection turns an agent's permissions into the attacker's. Allowing every shell command, every URL, or every tool call without a prompt means one malicious file or web page is enough to run code or send your source and secrets elsewhere. Shell plus network access together is the classic exfiltration chain.

## What triggers it
- Settings (`.claude/settings.json`): `permissions.allow` containing `Bash`, `Bash(*)`, `Bash(:*)` or `*`; `WebFetch` (or `WebFetch(*)`) without a domain; `defaultMode: "bypassPermissions"`; `enableAllProjectMcpServers: true` (Medium).
- Agent and prompt/command frontmatter: `tools` or `allowed-tools` with unrestricted `Bash` (Medium), or High when the same list also has `WebFetch` or `WebSearch`.
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

## Options
None.

## Suppressing
`<!-- pulse-ignore AI-SEC-007 -- reason -->` (Markdown) or a `[[ignore]]` entry in `pulse.toml`.
