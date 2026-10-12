# AI-XXX-NNN — Rule name

**Severity:** High · **Category:** Security · **Applies to:** MCP configs · **Inline-suppressible:** yes

## Why it matters
Two or three sentences on the concrete risk or cost, aimed at a developer who has never heard of this issue.

## What triggers it
- Bullet list of the detected patterns (user-facing wording of the spec's Detect section).

## Noncompliant
```json
{ "mcpServers": { "github": { "command": "npx", "args": ["-y", "@modelcontextprotocol/server-github"] } } }
```

## Compliant
```json
{ "mcpServers": { "github": { "command": "npx", "args": ["-y", "@modelcontextprotocol/server-github@2025.4.8"] } } }
```

## Options
`[rules.AI-XXX-NNN]` keys, or "None."

## Suppressing
`<!-- pulse-ignore AI-XXX-NNN -- reason -->` (Markdown) or a `[[ignore]]` entry in `pulse.toml`.
