#!/usr/bin/env bash
# Open a PR for the current branch against main. Body is read from stdin.
#   scripts/pr.sh "<title>" < body.md
# Uses `gh` when available; otherwise prints a compare URL with title and body prefilled.
set -euo pipefail
cd "$(dirname "$0")/.."
title=${1:?usage: scripts/pr.sh "<title>" < body.md}
body=$(cat)
branch=$(git rev-parse --abbrev-ref HEAD)
[[ $branch == main ]] && { echo "pr: on main, switch to a task branch first" >&2; exit 1; }

if command -v gh >/dev/null 2>&1; then
  gh pr create --base main --head "$branch" --title "$title" --body "$body"
  exit
fi

repo=$(git remote get-url origin | sed -E 's#(git@github.com:|https://github.com/)##; s#\.git$##')
enc() { printf '%s' "$1" | jq -sRr @uri; }
echo "https://github.com/$repo/compare/main...$branch?quick_pull=1&title=$(enc "$title")&body=$(enc "$body")"
