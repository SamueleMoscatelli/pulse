# Performance log

Budgets: `scripts/spec.sh "Performance budget"`. Measured with `scripts/bench.sh` (median wall time of `pulse scan -q`, release build, warm FS cache) and `cargo bench -p pulse-engine` (divan).

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
