#!/usr/bin/env bash
# Before/after comparison of two pulse binaries on real repos, as Markdown on stdout.
#   scripts/compare.sh [--old BIN] [--new BIN] <repo>...
# OLD defaults to `pulse` on PATH (the released version), NEW to target/release/pulse (built
# first). Per repo: totals old → new, per-rule finding counts sorted by |delta|, and the
# added/removed findings matched by fingerprint (at most 10 per rule here; the full lists go
# to target/compare/<repo-name>.md).
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)

old="" new=""
while [[ $# -gt 0 ]]; do
  case $1 in
    --old) old=${2:?--old needs a binary}; shift 2 ;;
    --new) new=${2:?--new needs a binary}; shift 2 ;;
    -h | --help) sed -n '2,7p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    -*) echo "compare: unknown option $1" >&2; exit 2 ;;
    *) break ;;
  esac
done
[[ $# -gt 0 ]] || { echo "usage: scripts/compare.sh [--old BIN] [--new BIN] <repo>..." >&2; exit 2; }

if [[ -z $old ]]; then
  old=$(command -v pulse) || { echo "compare: no pulse on PATH; pass --old BIN" >&2; exit 2; }
fi
if [[ -z $new ]]; then
  (cd "$root" && cargo build -q --release -p pulse-cli)
  new=$root/target/release/pulse
fi
out_dir=$root/target/compare
mkdir -p "$out_dir"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

# scan BIN REPO OUT: JSON report of every finding. A failing gate exits non-zero, which is
# a result, not an error; only a missing report is.
scan() {
  "$1" scan -q -f json --min-severity info -C "$2" >"$3" 2>"$tmp/err" || true
  [[ -s $3 ]] || { echo "compare: $1 produced no report for $2:" >&2; cat "$tmp/err" >&2; exit 1; }
}

for repo in "$@"; do
  [[ -d $repo ]] || { echo "compare: $repo is not a directory" >&2; exit 2; }
  name=$(basename "$(cd "$repo" && pwd)")
  scan "$old" "$repo" "$tmp/old.json"
  scan "$new" "$repo" "$tmp/new.json"
  python3 - "$name" "$tmp/old.json" "$tmp/new.json" "$out_dir/$name.md" <<'PY'
import json
import sys
from collections import Counter

name, old_path, new_path, full_path = sys.argv[1:]
old = json.load(open(old_path, encoding="utf-8"))
new = json.load(open(new_path, encoding="utf-8"))
SEVERITIES = ["critical", "high", "medium", "low", "info"]


def num(v):
    return round(v, 4) if isinstance(v, float) else v


def delta(a, b):
    if isinstance(a, (int, float)) and isinstance(b, (int, float)):
        d = num(b - a)
        return f"{d:+}" if d else "0"
    return "0" if a == b else "changed"


def cell(s):
    return str(s).replace("|", "\\|").replace("\n", " ")


def report(limit):
    lines = [f"## {name}", ""]
    lines.append(f"pulse {old['meta']['version']} → {new['meta']['version']}")
    lines += ["", "| | old | new | Δ |", "|---|---:|---:|---:|"]
    rows = [("assets", old["metrics"]["assets"], new["metrics"]["assets"])]
    rows.append(("findings", sum(old["metrics"]["issues"].values()), sum(new["metrics"]["issues"].values())))
    rows += [(s, old["metrics"]["issues"][s], new["metrics"]["issues"][s]) for s in SEVERITIES]
    rows.append(("gate", old["gate"]["status"], new["gate"]["status"]))
    rows.append(("context load", num(old["metrics"]["context_load"]), num(new["metrics"]["context_load"])))
    rows.append(("always-on tokens", old["metrics"]["always_on_tokens"], new["metrics"]["always_on_tokens"]))
    for label, a, b in rows:
        lines.append(f"| {label} | {a} | {b} | {delta(a, b)} |")

    old_rules = Counter(f["rule"] for f in old["findings"])
    new_rules = Counter(f["rule"] for f in new["findings"])
    rules = sorted(old_rules.keys() | new_rules.keys(),
                   key=lambda r: (-abs(new_rules[r] - old_rules[r]), r))
    lines += ["", "| rule | old | new | Δ |", "|---|---:|---:|---:|"]
    for r in rules:
        lines.append(f"| {r} | {old_rules[r]} | {new_rules[r]} | {delta(old_rules[r], new_rules[r])} |")

    old_fp = {f["fingerprint"] for f in old["findings"]}
    new_fp = {f["fingerprint"] for f in new["findings"]}
    for title, side, other in (("Added", new, old_fp), ("Removed", old, new_fp)):
        diff = [f for f in side["findings"] if f["fingerprint"] not in other]
        lines += ["", f"### {title} ({len(diff)})"]
        if not diff:
            lines += ["", "none"]
        by_rule = {}
        for f in diff:
            by_rule.setdefault(f["rule"], []).append(f)
        for r in sorted(by_rule, key=lambda r: (-len(by_rule[r]), r)):
            group = by_rule[r]
            lines += ["", f"**{r}** ({len(group)})", ""]
            for f in group[:limit]:
                lines.append(f"- `{f['path']}:{f['line']}` {cell(f['message'])}")
            if len(group) > limit:
                lines.append(f"- … {len(group) - limit} more in `target/compare/{name}.md`")
    return "\n".join(lines) + "\n"


print(report(10))
with open(full_path, "w", encoding="utf-8") as out:
    out.write(report(sys.maxsize))
PY
done
