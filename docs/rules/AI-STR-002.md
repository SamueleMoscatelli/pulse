# AI-STR-002 — Invalid file content

**Severity:** High (Info for skipped files) · **Category:** Structure · **Applies to:** all assets · **Inline-suppressible:** yes

## Why it matters
Agents read these files literally. A JSON syntax error can make a tool ignore the whole MCP config or settings file without any warning. Broken YAML frontmatter can drop a skill's `description`, so it never triggers. Pulse can't analyze what it can't parse, so the other rules see less of the file too.

## What triggers it
- Bytes that are not valid UTF-8 (Pulse decodes them as U+FFFD and keeps going).
- JSON/JSONC syntax errors in MCP configs, settings, hooks and tool definitions.
- Wrong shape for the kind, e.g. `mcpServers` is not an object or `permissions.allow` is not an array of strings.
- YAML frontmatter that does not parse into a mapping.
- Files over `scan.max_file_bytes`, or files that can't be read: reported at Info as "skipped", not analyzed.

## Noncompliant
```json
{ "mcpServers": ["github"] }
```

## Compliant
```json
{ "mcpServers": { "github": { "command": "npx", "args": ["-y", "@modelcontextprotocol/server-github@1.0.0"] } } }
```

## Options
None.

## Suppressing
`<!-- pulse-ignore AI-STR-002 -- reason -->` (Markdown) or a `[[ignore]]` entry in `pulse.toml`.
