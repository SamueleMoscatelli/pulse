# AI-SEC-006 — Insecure MCP transport

**Severity:** High (Medium for private network hosts) · **Category:** Security · **Applies to:** MCP configs · **Inline-suppressible:** yes

## Why it matters
Over plain `http://`, everything between the client and a remote MCP server travels unencrypted: auth headers, tool calls and their results, often including code and data from your workspace. Anyone on the network path can read or change it, and a tampered tool result is a prompt injection.

## What triggers it
- An HTTP or SSE server whose `url` (or `httpUrl`/`serverUrl`) uses `http://` and whose host is not loopback (`localhost`, `*.localhost`, `127.0.0.0/8`, `::1`).
- The severity is Medium when the host is a private IPv4 address (`10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`), High otherwise.

URLs whose host is a variable (`http://${env:HOST}/mcp`) are not checked.

## Noncompliant
```json
{ "mcpServers": { "docs": { "url": "http://docs-mcp.example.org/mcp" } } }
```

## Compliant
```json
{ "mcpServers": { "docs": { "url": "https://docs-mcp.example.org/mcp" } } }
```

## Options
None.

## Suppressing
MCP configs are JSON, so use an `[[ignore]]` entry in `pulse.toml`.
