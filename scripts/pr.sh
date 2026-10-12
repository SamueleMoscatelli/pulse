#!/usr/bin/env bash
# Open a PR for the current branch against main, using the HEAD commit's subject and body.
#   scripts/pr.sh
# Uses `gh` when available; otherwise prints the compare URL. With a single commit on the
# branch, GitHub fills the PR title and description from it.
set -euo pipefail
cd "$(dirname "$0")/.."
branch=$(git rev-parse --abbrev-ref HEAD)
[[ $branch == main ]] && { echo "pr: on main, switch to a task branch first" >&2; exit 1; }
n=$(git rev-list --count origin/main..HEAD)
[[ $n -ne 1 ]] && echo "pr: warning: $n commits on branch; GitHub prefills from the commit only when there is exactly one" >&2

if command -v gh >/dev/null 2>&1; then
  gh pr create --base main --head "$branch" --title "$(git log -1 --format=%s)" --body "$(git log -1 --format=%b)"
  exit
fi

repo=$(git remote get-url origin | sed -E 's#(git@github.com:|https://github.com/)##; s#\.git$##')
echo "https://github.com/$repo/compare/main...$branch?expand=1"
