#!/usr/bin/env bash
# PostToolUse hook: format the edited .rs file in place. Silent, never blocks.
f=$(jq -r '.tool_input.file_path // empty' 2>/dev/null)
[[ $f == *.rs && -f $f ]] || exit 0
cd "${CLAUDE_PROJECT_DIR:-.}" 2>/dev/null || exit 0
rustfmt --quiet "$f" >/dev/null 2>&1 || true
exit 0
