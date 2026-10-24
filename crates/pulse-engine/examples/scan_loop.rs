//! `scan_loop <DIR> [N]`: runs `scan()` N times (default 50) in one process, so a sampling
//! profiler sees a long-lived target. Build with `cargo build --profile profiling
//! -p pulse-engine --example scan_loop` (see `docs/perf.md`).

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use pulse_core::Config;
use pulse_engine::ScanOptions;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(dir) = args.next().map(PathBuf::from) else {
        eprintln!("usage: scan_loop <DIR> [N]");
        return ExitCode::from(2);
    };
    let n: u32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(50);
    let config = Config::default();
    let opts = ScanOptions::default();
    let start = Instant::now();
    for _ in 0..n {
        if let Err(e) = pulse_engine::scan(&dir, &config, &opts) {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    }
    eprintln!(
        "{n} scans, {:.2} ms each",
        start.elapsed().as_secs_f64() * 1e3 / f64::from(n)
    );
    ExitCode::SUCCESS
}
