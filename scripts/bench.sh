#!/usr/bin/env bash
# Release-build performance check against the budgets in docs/spec/00-overview.md.
# The budgets are for the reference machine (an Apple M-series laptop). Time budgets are
# scaled to this machine (ADR-038): a fixed calibration workload (`calibrate` example: a
# parallel walk of `typical` plus a parallel text pass, none of Pulse's code) is timed,
# and factor = calibration / BENCH_REF_MS, never below 1. Memory and size are not scaled.
# Exits 1 if any number exceeds its (scaled) budget by more than BENCH_SLACK (default
# 1.2 = +20%, for timer noise).
#   scripts/bench.sh          all scenarios
#   BENCH_ROUNDS=11 BENCH_RUNS=10 BENCH_SLACK=1 scripts/bench.sh
#   BENCH_FACTOR=1 scripts/bench.sh     no scaling: judge raw times against the spec
# Timing uses hyperfine when installed, else a Perl Time::HiRes loop (minimum of BENCH_RUNS).
set -euo pipefail
cd "$(dirname "$0")/.."

runs=${BENCH_RUNS:-5}
slack=${BENCH_SLACK:-1.2}
# Calibration time on the reference machine. PROVISIONAL: the i5-8257U's ~47 ms halved
# (M-series laptops run this kind of work about 2× faster); pin it with one run on an
# M-series laptop (docs/perf.md).
ref_ms=${BENCH_REF_MS:-24}
repos=target/bench-repos
bin=target/release/pulse
fail=0

cargo build -q --release -p pulse-cli
cargo build -q --release -p pulse-engine --example gen_repo --example calibrate

# gen NAME ASSETS FILES: synthesize a repo once and cache it under target/.
gen() {
  local dir=$repos/$1
  if [[ ! -f $dir/.done-$2-$3 ]]; then
    rm -rf "$dir" && mkdir -p "$dir"
    target/release/examples/gen_repo "$dir" "$2" "$3"
    touch "$dir/.done-$2-$3"
  fi
}

# best_ms CMD...: minimum wall time in milliseconds, like the calibration: on a shared
# machine other load only adds time (ADR-038).
best_ms() {
  if command -v hyperfine >/dev/null; then
    local json
    json=$(mktemp)
    # -i: a failing gate exits non-zero, which is not a timing error.
    hyperfine -N -i -w 3 -r "$runs" --export-json "$json" "$*" >/dev/null 2>&1
    grep -o '"min": *[0-9.e+-]*' "$json" | head -1 | awk -F: '{ printf "%.2f", $2 * 1000 }'
    rm -f "$json"
  else
    perl -MTime::HiRes=time -e '
      my ($n, @cmd) = @ARGV; my @t;
      open my $res, ">&", \*STDOUT; open STDOUT, ">", "/dev/null"; open STDERR, ">", "/dev/null";
      system(@cmd) for 1..3;
      for (1..$n) { my $s = time; system(@cmd); push @t, time - $s }
      @t = sort { $a <=> $b } @t; printf $res "%.2f", $t[0] * 1000;
    ' "$runs" "$@"
  fi
}

# peak_rss_mb CMD...: peak resident set size in MB.
peak_rss_mb() {
  local out div
  if [[ $(uname) == Darwin ]]; then
    out=$(/usr/bin/time -l "$@" 2>&1 >/dev/null | awk '/maximum resident set size/ { print $1 }')
    div=1048576
  else
    out=$(/usr/bin/time -v "$@" 2>&1 >/dev/null | awk -F: '/Maximum resident set size/ { print $2 }')
    div=1024
  fi
  # No reading (e.g. no GNU time on a minimal Linux image) must not pass as 0 MB.
  if [[ -z $out ]]; then
    echo "peak_rss_mb: /usr/bin/time gave no reading" >&2
    echo 999
    return
  fi
  awk -v v="$out" -v d="$div" 'BEGIN { printf "%.1f", v / d }'
}

# check NAME VALUE BUDGET UNIT
check() {
  local verdict
  verdict=$(awk -v v="$2" -v b="$3" -v s="$slack" 'BEGIN { if (v > b * s) print "FAIL"; else if (v > b) print "warn"; else print "ok" }')
  printf '%-34s %10s %-3s  budget < %s %s  %s\n' "$1" "$2" "$4" "$3" "$4" "$verdict"
  [[ $verdict != FAIL ]] || fail=1
}

gen typical 200 20000
gen large 1000 200000
# Hostile assets (roadmap P7.1): cheap to write, so regenerated every run.
rm -rf "$repos/hostile" && mkdir -p "$repos/hostile"
target/release/examples/gen_repo "$repos/hostile" hostile

echo "pulse $($bin --version | awk '{print $2}') · $(uname -sm) · $(sysctl -n machdep.cpu.brand_string 2>/dev/null || uname -p) · runs=$runs · $(command -v hyperfine >/dev/null && echo hyperfine || echo perl)"
# Interleaved rounds (ADR-038): each round calibrates (minimum of 5 runs per part), then
# times every scenario (minimum of BENCH_RUNS runs) and scales it to the reference
# machine: value / factor, factor = calibration / reference (min 1). The median over
# rounds is judged against the unscaled spec budgets. Interleaving cancels runner drift:
# calibrating only before and after let a CI run pass and the next fail on the same code.
rounds=${BENCH_ROUNDS:-7}
factors="" startups="" typicals="" larges="" hostiles=""
for ((r = 0; r < rounds; r++)); do
  read -r cal_walk cal_cpu threads < <(target/release/examples/calibrate "$repos/typical" 5)
  f=${BENCH_FACTOR:-$(awk -v w="$cal_walk" -v c="$cal_cpu" -v r="$ref_ms" 'BEGIN { f = (w + c) / r; printf "%.3f", (f < 1 ? 1 : f) }')}
  factors="$factors $f"
  # Startup: Pulse's own share, net of spawning a no-op process (OS cost, not ours).
  startups="$startups $(awk -v p="$(best_ms "$bin" --version)" -v t="$(best_ms /usr/bin/true)" -v f="$f" 'BEGIN { d = p - t; printf "%.3f", (d < 0 ? 0 : d) / f }')"
  typicals="$typicals $(awk -v v="$(best_ms "$bin" scan -q -C "$repos/typical")" -v f="$f" 'BEGIN { printf "%.3f", v / f }')"
  larges="$larges $(awk -v v="$(best_ms "$bin" scan -q -C "$repos/large")" -v f="$f" 'BEGIN { printf "%.3f", v / f }')"
  hostiles="$hostiles $(awk -v v="$(best_ms "$bin" scan -q -C "$repos/hostile")" -v f="$f" 'BEGIN { printf "%.3f", v / f }')"
done

# median VALUES...: the median, to 2 decimals.
median() { printf '%s\n' "$@" | sort -n | awk '{ a[NR] = $1 } END { printf "%.2f", a[int((NR + 1) / 2)] }'; }
# shellcheck disable=SC2086 # word splitting of the value lists is intended
{
  range=$(printf '%s\n' $factors | sort -n | awk 'NR == 1 { lo = $1 } { hi = $1 } END { printf "%.2f–%.2f", lo, hi }')
  echo "calibration ($threads threads, $rounds interleaved rounds): factor median $(median $factors), range $range · reference $ref_ms ms (provisional)"
  echo "time scenarios are scaled to the reference machine (measured / factor):"
  # Startup budget: 2 ms minus ~1 ms of process spawn on the reference machine.
  check "startup (--version, net of spawn)" "$(median $startups)" 1 ms
  check "typical (200 assets, 20k files)" "$(median $typicals)" 20 ms
  check "large (1k assets, 200k files)" "$(median $larges)" 150 ms
  # Robustness: 2× the typical rate for the same content, 2 × 20 ms × (5 MB / 2 MB).
  check "hostile (12 assets, ~5 MB)" "$(median $hostiles)" 100 ms
}
# Where typical's time goes (median of 10 `--timings` runs), for diagnosis only.
for _ in $(seq 10); do "$bin" scan -q --timings -C "$repos/typical" 2>&1 >/dev/null | tail -1; done |
  awk '{ for (i = 1; i <= NF; i++) if ($i ~ /^(discover|read|rules|total)$/) v[$i] = v[$i] " " $(i + 1) }
       END { printf "  typical phases (median):"; n = split("discover read rules total", k, " ")
             for (j = 1; j <= n; j++) { m = split(substr(v[k[j]], 2), a, " "); for (x = 1; x <= m; x++) for (y = x + 1; y <= m; y++) if (a[y] < a[x]) { t = a[x]; a[x] = a[y]; a[y] = t }
               printf " %s %.1f ms", k[j], a[int((m + 1) / 2)] } print "" }'
check "peak RSS (typical)" "$(peak_rss_mb "$bin" scan -q -C "$repos/typical")" 30 MB
check "binary size" "$(awk -v b="$(wc -c <"$bin")" 'BEGIN { printf "%.2f", b / 1048576 }')" 8 MB
exit $fail
