//! Robustness against hostile assets (roadmap P7.1): random bytes, huge lines, deeply
//! nested JSON/YAML/Markdown and 10k HTML comments must not panic, and must not blow the
//! time budget.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::{Duration, Instant};

use pulse_core::Config;
use pulse_engine::ScanOptions;

#[path = "../benches/support/hostile.rs"]
mod hostile;

/// 2× the typical budget for the same content, 2 × 20 ms × (5 MB / 2 MB) (ADR-038). Debug
/// builds are unoptimized, so they only guard against blow-ups (quadratic or worse), not
/// the budget; `scripts/bench.sh` checks the release number, scaled to the hardware.
fn budget() -> Duration {
    let release = Duration::from_millis(100);
    if cfg!(debug_assertions) {
        // ~5 s on an idle 2019 i5; loaded machines and CI's debug builds need headroom.
        // The blow-ups this guards against took minutes (339 s before P7.1).
        Duration::from_secs(60)
    } else {
        release
    }
}

#[test]
fn hostile_assets_scan_without_panic_within_budget() {
    let dir = tempfile::tempdir().unwrap();
    hostile::generate(dir.path()).unwrap();
    let config = Config::default();
    let opts = ScanOptions::default();

    // Warm the FS cache and the lazies, as the budget assumes.
    pulse_engine::scan(dir.path(), &config, &opts).unwrap();
    let start = Instant::now();
    let report = pulse_engine::scan(dir.path(), &config, &opts).unwrap();
    let took = start.elapsed();

    assert_eq!(report.assets_meta.len(), 12, "{:#?}", report.assets_meta);
    let empty: Vec<_> = report
        .assets_meta
        .iter()
        .filter(|a| a.tokens == 0)
        .map(|a| &a.rel_path)
        .collect();
    assert!(empty.is_empty(), "read, not skipped: {empty:?}");
    assert!(took < budget(), "scan took {took:?}, budget {:?}", budget());
}
