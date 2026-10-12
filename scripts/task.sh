#!/usr/bin/env bash
# Roadmap helper for /next-task.
#   scripts/task.sh next            -> card of the first unchecked task (with its phase goal)
#   scripts/task.sh show <ID>       -> card of a task, e.g. P2.3
#   scripts/task.sh done <ID>       -> tick the task in docs/plan/ROADMAP.md
#   scripts/task.sh status [--brief]
set -euo pipefail
cd "$(dirname "$0")/.."
roadmap=docs/plan/ROADMAP.md

next_id() { grep -m1 -oE '^- \[ \] P[0-9]+\.[0-9]+' "$roadmap" | awk '{print $4}' || true; }

card() {
  local id=$1 phase=${1%%.*}; phase=${phase#P}
  local file="docs/plan/phase-$phase.md"
  [[ -f $file ]] || { echo "No card file $file for $id" >&2; exit 1; }
  # phase goal (text between the title and the first card), then the card itself
  awk -v id="$id" '
    /^## P[0-9]+\.[0-9]+ / { incard = ($2 == id); intro = 0 }
    NR == 1 { intro = 1 }
    intro || incard { print }
  ' "$file"
  grep -qE "^## $id " "$file" || { echo "Card $id not found in $file" >&2; exit 1; }
}

case "${1:-status}" in
  next)
    id=$(next_id)
    [[ -n $id ]] || { echo "All roadmap tasks are done."; exit 0; }
    card "$id" ;;
  show)
    card "${2:?usage: task.sh show <ID>}" ;;
  done)
    id=${2:?usage: task.sh done <ID>}
    grep -qE "^- \[ \] $id " "$roadmap" || { echo "$id is not an open task" >&2; exit 1; }
    perl -pi -e "s/^- \[ \] \Q$id\E /- [x] $id /" "$roadmap"
    echo "Marked $id done." ;;
  status)
    total=$(grep -cE '^- \[[ x]\] P' "$roadmap" || true)
    done_n=$(grep -cE '^- \[x\] P' "$roadmap" || true)
    id=$(next_id)
    line=$( [[ -n $id ]] && grep -m1 -E "^- \[ \] $id " "$roadmap" | sed -E 's/^- \[ \] //' || echo "none")
    if [[ ${2:-} == --brief ]]; then
      echo "Pulse roadmap: $done_n/$total tasks done. Next: $line (run /next-task)."
    else
      echo "Done $done_n/$total. Next: $line"
      grep -E '^## Phase|^- \[' "$roadmap"
    fi ;;
  *) echo "usage: task.sh next | show <ID> | done <ID> | status [--brief]" >&2; exit 2 ;;
esac
