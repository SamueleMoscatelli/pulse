# AI-STR-006 — Invalid hook definition

**Severity:** High (timeouts and missing scripts: Medium) · **Category:** Structure · **Applies to:** settings and hook files · **Inline-suppressible:** yes

## Why it matters
A hook that the agent cannot run fails silently: a misspelled event never fires, a broken matcher never matches, and a script path that does not exist errors on every tool call. Guardrails you think are in place (formatting, linting, blocking dangerous commands) are simply not running.

## What triggers it
- An event name that the platform does not know (Claude Code: `PreToolUse`, `PostToolUse`, `Stop`, …; Cursor `.cursor/hooks.json`: `beforeShellExecution`, `afterFileEdit`, …). The message suggests the closest valid name.
- A `type` the platform does not support (Claude Code: `command`, `prompt`, `http`, `mcp_tool`, `agent`; Cursor: `command`, `prompt`).
- A `command` or `prompt` hook with no (or an empty) `command`/`prompt`.
- A matcher that is not a valid regular expression. `*`, `""` and plain lists like `Edit|Write` are fine; matchers using JavaScript-only syntax (look-around, backreferences) are not checked.
- `timeout` ≤ 0 or > 600 seconds (Medium).
- A script that does not exist (Medium): a path under `$CLAUDE_PROJECT_DIR/`, `${CLAUDE_PLUGIN_ROOT}/` (plugin hooks), `./…`, or a relative path with a script extension (`scripts/lint.py`). Paths with unexpanded variables or globs, `node_modules`, `..`, absolute paths and redirect targets are not checked. Files ignored by the scan are looked up on disk.

## Noncompliant
```json
{ "hooks": { "PreToolUser": [{ "matcher": "Edit(Write", "hooks": [{ "type": "command", "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/missing.sh", "timeout": 0 }] }] } }
```

## Compliant
```json
{ "hooks": { "PreToolUse": [{ "matcher": "Edit|Write", "hooks": [{ "type": "command", "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/check.sh", "timeout": 30 }] }] } }
```

## Options
None.

## Suppressing
A `[[ignore]]` entry in `pulse.toml` (JSON files have no inline comments).
