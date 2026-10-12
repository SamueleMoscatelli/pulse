#!/usr/bin/env bash
# The single verification gate: fmt + clippy + tests, with compact output (only what needs fixing).
#   scripts/check.sh            full check
#   scripts/check.sh -p CRATE   limit clippy/tests to one crate (faster inner loop)
set -uo pipefail
cd "$(dirname "$0")/.."
[[ -f Cargo.toml ]] || { echo "check: no Cargo workspace yet (task P0.1 creates it)"; exit 0; }

scope=(--workspace)
[[ ${1:-} == -p && -n ${2:-} ]] && scope=(-p "$2")
fail=0
max=${CHECK_MAX_LINES:-80}

cargo fmt --all --check >/dev/null 2>&1 || { cargo fmt --all; echo "fmt: reformatted files"; }

if ! out=$(cargo clippy "${scope[@]}" --all-targets -q --message-format short -- -D warnings 2>&1); then
  echo "clippy: FAILED"
  printf '%s\n' "$out" | grep -vE '^\s*(Compiling|Checking|Finished|Blocking)' | head -n "$max"
  exit 1
fi
echo "clippy: ok"

if ! out=$(cargo test "${scope[@]}" -q 2>&1); then
  fail=1
  echo "tests: FAILED"
  # failure details + summary, without the dot-progress noise
  printf '%s\n' "$out" | awk '/^failures:$/{p=1} /^error(\[|:)/{p=1} p' | grep -vE '^\s*(Compiling|Running|Doc-tests)' | head -n "$max"
  pending=$(find . -name '*.snap.new' -not -path './target/*' 2>/dev/null | head -5)
  [[ -n $pending ]] && echo "pending insta snapshots (review each diff before accepting):" && echo "$pending"
else
  passed=$(printf '%s\n' "$out" | grep -oE '[0-9]+ passed' | awk '{s+=$1} END{print s+0}')
  echo "tests: ok ($passed passed)"
fi
exit $fail
