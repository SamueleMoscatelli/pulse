# AI-EFF-005 — MCP server/tool sprawl

**Severity:** Medium (High for duplicate servers) · **Category:** Efficiency · **Applies to:** MCP configs, tool definitions (project-wide) · **Inline-suppressible:** yes

## Why it matters
Every enabled MCP server and tool adds its schema to the context and one more option for the model to choose from. Past a few dozen tools, selection gets slower, costlier and less accurate. The same server defined twice under different names doubles that cost for nothing.

## What triggers it
- More enabled MCP servers than `budgets.mcp_servers` (default 8). A server name defined in several clients' configs counts once; servers with `"disabled": true` or `"enabled": false` are not counted.
- More tool definitions across Tools assets than `budgets.tools_count` (default 40). Token waste: the tokens of the definitions past the budget.
- Two servers with different names and the same launch target (High). Commands are compared by file name (`/usr/local/bin/npx` = `npx`) with npx's `-y`/`--yes` ignored; URLs ignore scheme/host case and a trailing `/`.

## Noncompliant
```json
{ "mcpServers": {
  "github": { "command": "npx", "args": ["-y", "@modelcontextprotocol/server-github@2025.4.8"] },
  "gh": { "command": "npx", "args": ["@modelcontextprotocol/server-github@2025.4.8"] }
} }
```

## Compliant
```json
{ "mcpServers": {
  "github": { "command": "npx", "args": ["-y", "@modelcontextprotocol/server-github@2025.4.8"] }
} }
```

## Options
None. Limits are set in `[budgets]` (`mcp_servers`, `tools_count`) in `pulse.toml`.

## Suppressing
A `[[ignore]]` entry in `pulse.toml` (JSON has no inline comments).
