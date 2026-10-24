//! `calibrate <DIR>`: times a fixed workload shaped like a scan, to scale the performance
//! budgets to the machine running `scripts/bench.sh` (ADR-038). It uses none of Pulse's
//! code, so a slower Pulse can't hide behind a slower calibration.
//!
//! `calibrate <DIR> [RUNS]` prints `walk_ms cpu_ms threads` (minimum of RUNS runs each,
//! default 15): a parallel walk of `DIR` with
//! Pulse's walker settings, and a parallel char/word-hash pass over 32 MB of synthetic
//! prose that allocates nothing (an allocating pass ran 2× slower on CI's VM while Pulse
//! ran faster there, ADR-038).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use ignore::{WalkBuilder, WalkState};
use rayon::prelude::*;
use xxhash_rust::xxh3::xxh3_64;

/// Default runs per part; the minimum is kept. On shared machines (CI) other load only
/// ever adds time, so the minimum estimates the machine itself; a median swung 7× between
/// two CI runs (ADR-038).
const RUNS: usize = 15;

fn main() -> ExitCode {
    let Some(dir) = std::env::args().nth(1).map(PathBuf::from) else {
        eprintln!("usage: calibrate <DIR> [RUNS]");
        return ExitCode::from(2);
    };
    let runs = std::env::args()
        .nth(2)
        .and_then(|n| n.parse().ok())
        .filter(|&n| n > 0)
        .unwrap_or(RUNS);
    let text = prose();
    walk(&dir);
    cpu(&text);
    let walk_ms = fastest(runs, || walk(&dir));
    let cpu_ms = fastest(runs, || cpu(&text));
    let threads = std::thread::available_parallelism().map_or(1, usize::from);
    match writeln!(std::io::stdout(), "{walk_ms:.2} {cpu_ms:.2} {threads}") {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}

/// Minimum wall time of `f` over `runs` runs, in milliseconds.
fn fastest(runs: usize, mut f: impl FnMut() -> usize) -> f64 {
    let t: Vec<f64> = (0..runs)
        .map(|_| {
            let start = Instant::now();
            std::hint::black_box(f());
            start.elapsed().as_secs_f64() * 1e3
        })
        .collect();
    t.into_iter().fold(f64::INFINITY, f64::min)
}

/// Files walked, with the walker settings of `pulse_core::discover`.
fn walk(dir: &Path) -> usize {
    let files = AtomicUsize::new(0);
    WalkBuilder::new(dir)
        .hidden(false)
        .git_ignore(true)
        .git_global(false)
        .require_git(false)
        .parents(false)
        .build_parallel()
        .run(|| {
            let files = &files;
            Box::new(move |entry| {
                if entry.is_ok_and(|e| e.file_type().is_some_and(|t| t.is_file())) {
                    files.fetch_add(1, Ordering::Relaxed);
                }
                WalkState::Continue
            })
        });
    files.into_inner()
}

/// 32 MB of mixed prose in 64 KB chunks (long enough that scheduling noise is small), like many small instruction files.
fn prose() -> Vec<String> {
    let para = "## Conventions\n\nAlways prefer explicit error types; never panic. Résumé \
                of the café rules: “quote” — dash, `code`, https://example.dev/x.\n\n";
    (0..512)
        .map(|i| format!("# File {i}\n\n{}", para.repeat(64 * 1024 / para.len())))
        .collect()
}

/// Rule-like work per chunk, without allocating (as Pulse's tokenizer works): classify
/// every char, and hash each word in place, ASCII-lowercased.
fn cpu(chunks: &[String]) -> usize {
    chunks
        .par_iter()
        .map(|c| {
            let mut acc = 0u64;
            let mut start = None;
            for (i, ch) in c.char_indices() {
                if ch.is_alphanumeric() {
                    start.get_or_insert(i);
                } else if let Some(s) = start.take() {
                    let w = &c.as_bytes()[s..i];
                    let lower = w.iter().fold(0u64, |h, b| {
                        h.rotate_left(5) ^ u64::from(b.to_ascii_lowercase())
                    });
                    acc = acc.wrapping_add(xxh3_64(w) ^ lower);
                }
            }
            (acc & 1) as usize
        })
        .sum()
}
