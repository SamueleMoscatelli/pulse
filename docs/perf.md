# Performance log

Budgets: `scripts/spec.sh "Performance budget"`. Measured with `scripts/bench.sh` (minimum wall time of `pulse scan -q` since P7.1, median before; release build, warm FS cache) and `cargo bench -p pulse-engine` (divan).

## P7.1 — 2026-10-24

Profiled with `cargo build --profile profiling -p pulse-engine --example scan_loop` and macOS `sample` (`scan_loop <DIR> N` repeats `scan()` in one process). Each fix below keeps the JSON output byte-identical on `typical`, `large` and `hostile` (pre-task binary vs post-task binary).

Top 3 hotspots on `typical` (share of CPU samples, i5-8257U):

| Hotspot | Fix |
|---|---|
| AI-COG-002 `vague()`: split and lemmatize every `VAGUE`/`ANCHORS` phrase for every word of every sentence | phrases lemmatized once (`LazyLock`) |
| Discovery `rel_of`: `Path::strip_prefix` compares components for each of the 20k–200k files | byte-prefix fast path |
| Discovery `all_paths`: one `HashSet` insert per file per thread, then a re-hash of every path when the threads merge | per-thread `Vec`, one pre-sized `HashSet` |

Robustness corpus (`hostile`, ADR-037): 42 s → 0.26 s total on the i5. Blow-ups fixed: deep YAML flow nesting (libyaml, 39 s), AI-COG-001's pairwise directive loop, and the per-finding costs on one huge line (fingerprint line copy and hash, column char count, `Path` compares in `sort`/`dedup`). `contains_phrase` and `words()` got cheaper too.

`scripts/bench.sh`, i5-8257U (not the reference hardware; the machine was under background load, 3–28):

| Scenario | Measured | Budget |
|---|---|---|
| Startup | 5.51 ms | < 2 ms |
| Typical | 58.72 ms | < 20 ms |
| Large | 336.64 ms | < 150 ms |
| Hostile (12 assets, ~5 MB) | 338.82 ms | < 40 ms |
| Peak RSS (typical) | 13.0 MB | < 30 MB |
| Binary size | 4.00 MB | < 8 MB |

Apple M-series, CI `bench` job (`Apple M2 Pro (Virtual)`, hyperfine, first PR run, before the prose cache below):

| Scenario | Measured | Budget | Verdict |
|---|---|---|---|
| Startup | 1.41 ms | < 2 ms | ok |
| Typical | 38.91 ms | < 20 ms | over |
| Large | 169.84 ms | < 150 ms | over (within 20%) |
| Hostile | 166.32 ms | < 40 ms | over |
| Peak RSS (typical) | 15.0 MB | < 30 MB | ok |
| Binary size | 3.45 MB | < 8 MB | ok |

The CI VM is not the reference laptop: it runs `typical` about as fast as the 2019 i5 (39 vs 45 ms), and the walk (about half of `typical`) is already faster than `rg --files` on the same tree.

Second pass: sentences and words are segmented once per asset (`Asset::prose()`, `pulse_core::text::prose`) instead of by each of six prose rules, and `normalize` has an ASCII fast path. Median `rules` phase on the i5 (15 interleaved runs): typical 22.0 → 19.8 ms, large 60.3 → 50.7 ms, hostile 232.8 → 182.3 ms. Output unchanged.

Third pass: one-time setup. A one-file scan still spent ~7 ms in `rules` building lexicons (secrets 3.6 ms, tool-call regex 2.7 ms, injection 2.4 ms…). Fix: build them on the rayon pool during the walk (ADR-039), and compile secret regexes on demand. Medians on the i5, 15 interleaved runs (25 for large):

| Repo | `rules` before → after | total before → after |
|---|---|---|
| One small file | 6.6 → 0.3 ms | 12.5 → 6.2 ms |
| Typical | 20.0 → 9.6 ms | 43.5 → 33.1 ms |
| Large | 55.5 → 42.8 ms | 232.5 → 218.1 ms |
| Hostile | 148.3 → 135.2 ms | 205.5 → 191.3 ms |

The same pass also hashes AI-COG-001's index keys instead of allocating strings, lemmatizes its exclusive sets once, and scans sentence delimiters by byte.

### Hardware-calibrated budgets (ADR-038)

`bench.sh` scales time budgets by machine speed (calibration ÷ provisional reference of 15 ms).

First calibration (allocating CPU pass) on CI, `Apple M2 Pro (Virtual)`: walk 19.78 + cpu 33.44 = 53.2 ms, factor 2.80, every scenario ok (typical 34.78 ms, large 157.76 ms, hostile 118.12 ms, startup net 0.63 ms). On the i5 the same calibration measured 35.0 ms while Pulse's typical took 53.9 ms. So the CI VM looked 1.5× slower by calibration but ran Pulse 1.5× faster, which made its budgets about 2× too lenient. The CPU pass was replaced with an allocation-free one.

Allocation-free calibration, i5-8257U under background load (load average 5–8), calibration 34.8 ms, factor 2.32:

| Scenario | Measured | Scaled budget | Verdict |
|---|---|---|---|
| Startup, net of spawn | 3.24 ms | < 2.3 ms | over (noisy: a difference of two spawn timings) |
| Typical | 58.11 ms | < 46.4 ms | over |
| Large | 343.36 ms | < 348.0 ms | ok |
| Hostile | 261.47 ms | < 232.0 ms | warn |
| Peak RSS / binary | 13.4 MB / 4.00 MB | < 30 / < 8 MB | ok |

CI after ADR-039 (busier runner): calibration walk 24.70 + cpu 31.70 = 56.4 ms (cpu was 4.50 ms the run before), factor 3.76, all ok: typical 38.61 ms, large 201.27 ms, hostile 128.99 ms. The 7× swing in the CPU part made that factor meaningless, so the calibration now takes the minimum of 15 runs over a 32 MB pass, before and after the scenarios (reference re-estimated at 24 ms).

i5, final calibration (load average 5–8), 46.7 ms, factor 1.95:

| Scenario | Measured | Scaled budget | Verdict |
|---|---|---|---|
| Startup, net of spawn | 2.47 ms | < 1.9 ms | over (OS loader; 0.7 ms on CI) |
| Typical | 43.68 ms | < 39.0 ms | warn |
| Large | 250.14 ms | < 292.5 ms | ok |
| Hostile | 214.75 ms | < 195.0 ms | warn |
| Peak RSS / binary | 12.9 MB / 4.00 MB | < 30 / < 8 MB | ok |

CI with minimum timings and phases: calibration 29.5 ms (factor 1.23); typical 30.38 ms against 24.6 (over by 23.5%, just past the 20% slack); large and hostile ok. Phases: discover 23.4 ms, read 2.0, rules 5.4. The calibration's bare walk of the same tree is 13.1 ms, so discovery's own work is the gap. On the i5, walker settings (`.pulseignore`, the exclude filter) cost nothing and a fresh process is not slower; the per-file visit is the gap: classifying each path against the `GlobSet` (~28% of it), building its relative path, and recording it. Fix: a file-name prefilter derived from the patterns' last segments skips the `GlobSet` for files that cannot be assets (most of a repo).

**Final CI run** (`Apple M2 Pro (Virtual)`, 5 threads, hyperfine): calibration walk 16.87 + cpu 15.00 = 31.9 ms, factor 1.33. Every budget met:

| Scenario | Measured | Scaled budget | Verdict |
|---|---|---|---|
| Startup, net of spawn | 0.81 ms | < 1.3 ms | ok |
| Typical | 24.59 ms | < 26.6 ms | ok |
| Large | 146.34 ms | < 199.5 ms | ok |
| Hostile | 95.99 ms | < 133.0 ms | ok |
| Peak RSS (typical) | 15.0 MB | < 30 MB | ok |
| Binary size | 3.45 MB | < 8 MB | ok |

Typical phases (median): discover 17.7 ms (23.4 before the prefilter), read 2.0, rules 5.1. Across the sane CI runs the factor stayed within 1.23–1.49.

The next CI run, same code: calibration walk 11.28 + cpu 15.53 = 26.8 ms, factor 1.12; typical 26.95 ms against 22.4 (FAIL by 0.05 ms past the slack), the rest ok; phases discover 21.7, read 1.5, rules 4.1. The runner's speed drifted within the run, so a calibration taken only before and after the scenarios flipped the verdict. `bench.sh` now interleaves: 7 rounds of calibrate-then-measure, each time scaled to the reference machine, median judged against the spec budgets.

Interleaved, i5 under heavy load (load average 10–11), two consecutive runs:

| Scenario (scaled to reference) | Run 1 | Run 2 | Budget |
|---|---|---|---|
| Startup, net of spawn | 1.26 ms | 1.15 ms | < 1 ms (i5 loader) |
| Typical | 21.61 ms | 20.74 ms | < 20 ms (warn) |
| Large | 122.06 ms | 122.20 ms | < 150 ms |
| Hostile | 107.22 ms | 110.42 ms | < 100 ms (warn) |
| Factor range | 1.89–2.03 | 1.95–2.56 | |

**Final CI run, interleaved** (`Apple M2 Pro (Virtual)`, 5 threads, hyperfine): factor median 1.59, range 1.15–1.71 within the run (the drift that interleaving cancels). All budgets met:

| Scenario (scaled to reference) | Measured | Budget | Verdict |
|---|---|---|---|
| Startup, net of spawn | 0.60 ms | < 1 ms | ok |
| Typical | 19.32 ms | < 20 ms | ok |
| Large | 88.51 ms | < 150 ms | ok |
| Hostile | 70.55 ms | < 100 ms | ok |
| Peak RSS (typical) | 15.4 MB | < 30 MB | ok |
| Binary size | 3.45 MB | < 8 MB | ok |

Typical phases (median, raw): discover 21.1 ms, read 1.6, rules 4.7. Typical sits at its budget on every machine measured: 19.3 ms on CI and 20.7–21.6 ms on the i5 (warn), so expect ok or warn, not FAIL. Discovery is the remaining lever.

The reference calibration (24 ms) is still provisional: pin `BENCH_REF_MS` with one `scripts/bench.sh` run on an M-series laptop (its printed calibration time; backlog).

## v0.1.0 — 2026-10-15

Machine: MacBook Pro 2019, Intel Core i5-8257U @ 1.40 GHz (4 cores / 8 threads), 8 GB RAM, macOS 15 (Darwin 24.6.0), APFS. Timing: Perl fallback (no hyperfine), 20 runs, 3 warmups. Not the reference hardware (Apple M-series): see ADR-019.

| Scenario | Measured | Budget | Verdict |
|---|---|---|---|
| Startup (`pulse --version`) | 6.12 ms | < 2 ms | over |
| Typical (200 assets, 20k files walked) | 45.68 ms | < 20 ms | over |
| Large (1,000 assets, 200k files walked) | 316.66 ms | < 150 ms | over |
| Peak RSS (typical) | 8.2 MB | < 30 MB | ok |
| Release binary size | 2.82 MB | < 8 MB | ok |

Reference points on the same machine:

| | typical | large |
|---|---|---|
| Spawn `/usr/bin/true` | 4.15 ms | 4.15 ms |
| `rg --files` (same `ignore` walker) | ~40 ms | ~380 ms |
| `pulse --timings`: discover · read · rules · total | 28 · 2 · 6 · 37 ms | 190 · 7 · 20 · 225 ms |

divan (`cargo bench -p pulse-engine`), medians:

| Bench | Median | Throughput |
|---|---|---|
| `estimator` (64 KB) | 85.5 µs | 766 MB/s |
| `normalize` (64 KB) | 1.30 ms | 50 MB/s |
| `scan_large` (1,000 assets, 10k files, in-process) | 61.2 ms | n/a |
| `duplicated_content` (AI-EFF-003, 1,000 assets × 20 paragraphs, views warm; P5.1, 2026-10-19) | 10.9 ms (34 ms on 1 thread) | n/a |

With AI-EFF-003 (P5.1), the `large` fixture reports 1,600 duplicate paragraphs (each package repeats its paragraphs across CLAUDE.md, AGENTS.md, agent, command and skill). The rule adds about 10–15 ms to `rules` there, and handling its findings adds about 15 ms more (i5-8257U, `--timings`).

Dogfooding: `pulse ci` on this repo scans 42 assets in about 12 ms and the gate passes. Our own `.claude/` assets have no findings. The two deliberately malformed test fixtures are ignored in `pulse.toml`.
