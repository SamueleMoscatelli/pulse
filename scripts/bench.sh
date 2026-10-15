#!/usr/bin/env bash
# Release-build performance check against the budgets in docs/spec/00-overview.md.
# Fails if any number exceeds its budget by more than 20%.
#   scripts/bench.sh          all scenarios
#   BENCH_RUNS=50 scripts/bench.sh
# Timing uses hyperfine when installed, else a Perl Time::HiRes loop (median of BENCH_RUNS).
set -euo pipefail
cd "$(dirname "$0")/.."

runs=${BENCH_RUNS:-20}
repos=target/bench-repos
bin=target/release/pulse
fail=0

cargo build -q --release -p pulse-cli
cargo build -q --release -p pulse-engine --example gen_repo

# gen NAME ASSETS FILES: synthesize a repo once and cache it under target/.
gen() {
  local dir=$repos/$1
  if [[ ! -f $dir/.done-$2-$3 ]]; then
    rm -rf "$dir" && mkdir -p "$dir"
    target/release/examples/gen_repo "$dir" "$2" "$3"
    touch "$dir/.done-$2-$3"
  fi
}

# median_ms CMD...: median wall time in milliseconds.
median_ms() {
  if command -v hyperfine >/dev/null; then
    local json
    json=$(mktemp)
    hyperfine -N -w 3 -r "$runs" --export-json "$json" "$*" >/dev/null 2>&1
    grep -o '"median": *[0-9.e+-]*' "$json" | head -1 | awk -F: '{ printf "%.2f", $2 * 1000 }'
    rm -f "$json"
  else
    perl -MTime::HiRes=time -e '
      my ($n, @cmd) = @ARGV; my @t;
      open my $res, ">&", \*STDOUT; open STDOUT, ">", "/dev/null"; open STDERR, ">", "/dev/null";
      system(@cmd) for 1..3;
      for (1..$n) { my $s = time; system(@cmd); push @t, time - $s }
      @t = sort { $a <=> $b } @t; printf $res "%.2f", $t[$#t / 2] * 1000;
    ' "$runs" "$@"
  fi
}

# peak_rss_mb CMD...: peak resident set size in MB.
peak_rss_mb() {
  local out
  if [[ $(uname) == Darwin ]]; then
    out=$(/usr/bin/time -l "$@" 2>&1 >/dev/null | awk '/maximum resident set size/ { print $1 }')
    awk -v b="$out" 'BEGIN { printf "%.1f", b / 1048576 }'
  else
    out=$(/usr/bin/time -v "$@" 2>&1 >/dev/null | awk -F: '/Maximum resident set size/ { print $2 }')
    awk -v k="$out" 'BEGIN { printf "%.1f", k / 1024 }'
  fi
}

# check NAME VALUE BUDGET UNIT
check() {
  local verdict
  verdict=$(awk -v v="$2" -v b="$3" 'BEGIN { if (v > b * 1.2) print "FAIL"; else if (v > b) print "warn"; else print "ok" }')
  printf '%-34s %10s %-3s  budget < %s %s  %s\n' "$1" "$2" "$4" "$3" "$4" "$verdict"
  [[ $verdict != FAIL ]] || fail=1
}

gen typical 200 20000
gen large 1000 200000

echo "pulse $($bin --version | awk '{print $2}') · $(uname -sm) · runs=$runs · $(command -v hyperfine >/dev/null && echo hyperfine || echo perl)"
check "startup (--version)" "$(median_ms "$bin" --version)" 2 ms
check "typical (200 assets, 20k files)" "$(median_ms "$bin" scan -q -C "$repos/typical")" 20 ms
check "large (1k assets, 200k files)" "$(median_ms "$bin" scan -q -C "$repos/large")" 150 ms
check "peak RSS (typical)" "$(peak_rss_mb "$bin" scan -q -C "$repos/typical")" 30 MB
check "binary size" "$(awk -v b="$(wc -c <"$bin")" 'BEGIN { printf "%.2f", b / 1048576 }')" 8 MB
exit $fail
