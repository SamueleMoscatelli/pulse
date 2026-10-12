#!/usr/bin/env bash
# Print spec sections by heading, so agents read only what they need.
#   scripts/spec.sh                 -> table of contents (file:line  heading)
#   scripts/spec.sh "AI-SEC-002"    -> every section whose heading contains the query (case-insensitive),
#                                      up to the next heading of the same or higher level
set -euo pipefail
cd "$(dirname "$0")/.."
files=(docs/spec/*.md)

if [[ $# -eq 0 ]]; then
  awk '/^```/{f=!f} !f && /^#+ /{print FILENAME ":" FNR "  " $0}' "${files[@]}"
  exit 0
fi

out=$(awk -v q="$*" '
  FNR == 1 { p = 0; f = 0 }
  /^```/ { f = !f }
  !f && /^#+ / {
    match($0, /^#+/); lvl = RLENGTH
    if (p && lvl <= plvl) p = 0
    if (!p && lvl >= 2 && index(tolower($0), tolower(q))) { p = 1; plvl = lvl; print "==> " FILENAME ":" FNR }
  }
  p { print }
' "${files[@]}")

if [[ -z "$out" ]]; then
  echo "No section matches '$*'. Run scripts/spec.sh for the table of contents." >&2
  exit 1
fi
printf '%s\n' "$out"
