# AI-SEC-003 — Hardcoded secret

**Severity:** Critical · **Category:** Security · **Applies to:** all assets · **Inline-suppressible:** no

## Why it matters
AI assets are committed, shared and pasted into prompts, so a key written into one leaks to everyone who can read the repo and to every model that loads the file. A leaked key must be treated as compromised: revoke it, don't just delete the line.

## What triggers it
- Provider key formats: AWS access key IDs (`AKIA…`), GitHub tokens (`ghp_…`, `gho_…`, `ghu_…`, `ghs_…`, `ghr_…`, `github_pat_…`), Anthropic (`sk-ant-…`), OpenAI (`sk-…`, `sk-proj-…`, with mixed case and digits), Slack (`xoxb-…` and friends), Google API keys (`AIza…`), Stripe live keys (`sk_live_…`), PEM private-key headers and JWTs (`eyJ….eyJ….…`).
- A generic assignment such as `db_password = …`, `"apiKey": "…"` or `API_TOKEN: …` whose value is at least 12 characters, contains a digit and looks random (Shannon entropy ≥ 3.5 bits/char). Values that look like code (`config.get_api_key()`, `settings.SECRET_KEY`, `os.environ[…]`), URLs (`https://…`) and word paths (`Desktop/Title/H1`) are skipped.

The message names the provider and shows only the first 4 characters, e.g. `Possible OpenAI secret "sk-p…"`. Obvious placeholders are ignored: values containing `xxx`, `<…>`, `${…}`, `{{…}}`, `your-`, `example`, `dummy`, `changeme`, `placeholder`, `redacted`, environment lookups (`$VAR`, `process.env`, `os.environ`, `getenv`), and values that are mostly one repeated character.

## Noncompliant
```json
{ "mcpServers": { "openai": { "command": "npx", "args": ["-y", "openai-mcp@1.4.2"],
  "env": { "OPENAI_API_KEY": "sk-proj-…the real 40+ character key…" } } } }
```

## Compliant
```json
{ "mcpServers": { "openai": { "command": "npx", "args": ["-y", "openai-mcp@1.4.2"],
  "env": { "OPENAI_API_KEY": "${env:OPENAI_API_KEY}" } } } }
```

## Options
None.

## Suppressing
Not inline-suppressible. For a known false positive, add an `[[ignore]]` entry in `pulse.toml`.
