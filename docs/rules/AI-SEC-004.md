# AI-SEC-004 — MCP credential not externalized

**Severity:** High · **Category:** Security · **Applies to:** MCP configs · **Inline-suppressible:** yes

## Why it matters
MCP config files (`.mcp.json`, `.vscode/mcp.json`, `.cursor/mcp.json`, `.kiro/settings/mcp.json`) are usually committed and shared. A token written into one is readable by everyone with access to the repo, ends up in backups and forks, and can't be rotated per developer. Clients can resolve environment variables and prompted inputs at launch, so the value never needs to be in the file.

## What triggers it
- An `env`, `headers` or Kiro `oauth` entry (`clientSecret`) whose key contains `key`, `token`, `secret`, `password`, `auth` or `bearer` (any case), with a literal value.
- A command-line argument after such a flag, either `--api-key value` or `--api-key=value`.
- A Docker env argument, `-e GITHUB_TOKEN=value` or `--env=GITHUB_TOKEN=value`.

A value is not literal when it is empty, references a variable (`${env:X}`, `${input:X}`, `${X}`, `$X`, also inside text such as `Bearer ${input:token}`), is an obvious placeholder (`<your-token>`, `changeme`, …), is a switch or a number (`true`, `off`, `4096`), or is a file path or a URL without credentials in it. Keys that name where a credential lives or how it is used (`API_KEY_FILE`, `AUTH_URL`, `AUTH_TYPE`, `--key-file`) are not checked. The message names the key and server, never the value. Servers marked `disabled: true` are checked too: the file is committed either way. When the value is also a recognizable secret, only [AI-SEC-003](AI-SEC-003.md) is reported.

## Noncompliant
```json
{ "mcpServers": { "jira": { "type": "http", "url": "https://jira.example.com/mcp",
  "headers": { "Authorization": "Bearer abc123def456" } } } }
```

## Compliant
```json
{ "inputs": [{ "type": "promptString", "id": "jira-token", "password": true }],
  "servers": { "jira": { "type": "http", "url": "https://jira.example.com/mcp",
  "headers": { "Authorization": "Bearer ${input:jira-token}" } } } }
```

## Options
None.

## Suppressing
MCP configs are JSON, so use an `[[ignore]]` entry in `pulse.toml`.
