# Phase 4 — Structure & validity pack

Goal: catch "this asset will silently not work" bugs. Every task follows the `add-rule` skill.

## P4.1 — AI-STR-001 & AI-STR-004 frontmatter schemas
- **Size** M · **Depends** P3.6
- **Read** `scripts/spec.sh "AI-STR-001"`, `scripts/spec.sh "AI-STR-004"`; skill `add-rule`
- **Do** `structure/schemas.rs`: per-(kind, platform) required/known key tables (Claude skill, Claude agent, Claude command, Cursor `.mdc`, Copilot `.instructions.md`/`.prompt.md`/`.agent.md`). Platform comes from the path (`.claude/`, `.cursor/`, `.github/`). Levenshtein helper in `pulse-core::text` (bounded, early exit).
- **Done when** check is green; fixtures for each platform's valid file (pass), a missing description, a typo key (`descripton` → Low + suggestion), an invalid skill name, and a name/dir mismatch.

## P4.2 — AI-STR-006 & AI-STR-005 hooks & tool names
- **Size** M · **Depends** P4.1
- **Read** `scripts/spec.sh "AI-STR-006"`, `scripts/spec.sh "AI-STR-005"`; skill `add-rule`
- **Do** `lexicon/platform.rs`: `HOOK_EVENTS` (Claude Code: PreToolUse, PostToolUse, UserPromptSubmit, Notification, Stop, SubagentStop, PreCompact, SessionStart, SessionEnd, and any newer ones; check current docs if network is available, otherwise use this list and leave a decision note), `KNOWN_TOOLS` per platform. AI-STR-005 needs the MCP server names from all McpConfig assets, so implement it as a project rule.
- **Done when** check is green; fixtures: unknown event, bad matcher regex, missing script, `mcp__github__create_issue` with/without a `github` server defined, `Reed` → suggests `Read`.

## P4.3 — AI-STR-003 references & import graph
- **Size** L · **Depends** P4.2
- **Read** `scripts/spec.sh "AI-STR-003"`, `scripts/spec.sh "AI-EFF-002"`; skill `add-rule`
- **Do** `pulse-engine` (or `pulse-rules/src/structure/refs.rs`): resolve `@imports` and relative links against `Discovery.all_paths` (exposed via `RuleCtx`); build the import graph (max depth 5, cycle detection). Imported files that aren't assets get read and token-estimated (cap them by `max_file_bytes`). Feed resolved import tokens into AI-EFF-002's always-on total (resolve the `// P4.3:` note).
- **Done when** check is green; fixtures: broken `@docs/missing.md`, a valid import, a cycle a→b→a (High), a link with `#anchor` (pass), an example path in a code fence (pass); an EFF-002 test showing imported tokens counted.

## P4.4 — AI-STR-007 duplicate asset names
- **Size** S · **Depends** P4.3
- **Read** `scripts/spec.sh "AI-STR-007"`; skill `add-rule`
- **Do** Project rule; invocation-name derivation per kind/platform (`.claude/commands/git/commit.md` → `/git:commit`).
- **Done when** check is green; fixtures for duplicate skill names in different dirs, and the same name across different platforms (pass).
