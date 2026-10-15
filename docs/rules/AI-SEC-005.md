# AI-SEC-005 — Unpinned MCP server package

**Severity:** High · **Category:** Security · **Applies to:** MCP configs · **Inline-suppressible:** yes

## Why it matters
An MCP server runs on your machine with your credentials. When the config launches it with `npx some-server` or `uvx some-server`, every start fetches whatever version the registry serves at that moment, so a compromised or hijacked release runs automatically with no review. Pinning an exact version (or an image digest) makes upgrades a deliberate, reviewable change.

## What triggers it
- `npx`, `bunx`, `pnpm dlx`, `yarn dlx` with a package (positional or `--package`) that has no exact `@MAJOR.MINOR.PATCH` version: no version, `@latest`, a range (`@^1`, `@~1.2`, `@1`) or a dist-tag (`@next`). With `-y`/`--yes` the message adds "(auto-install)".
- `uvx`, `uv tool run`, `pipx run` with a package (positional, `--from` or `--spec`) without an exact `==version` or `@1.2.3` version: a bare name, `@latest`, `@1`, or a range (`>=1.0`).
- `docker run` with an image that has no tag or digest, or is tagged `:latest`.

Local paths, URLs, `git+…`/`github:` specs and variable references are not checked.

## Noncompliant
```json
{ "mcpServers": { "github": { "command": "npx", "args": ["-y", "@modelcontextprotocol/server-github"] } } }
```

## Compliant
```json
{ "mcpServers": { "github": { "command": "npx", "args": ["-y", "@modelcontextprotocol/server-github@2025.4.8"] } } }
```

## Options
None.

## Suppressing
MCP configs are JSON, so use an `[[ignore]]` entry in `pulse.toml`.
