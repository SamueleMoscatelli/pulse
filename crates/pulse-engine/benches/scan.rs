//! Throughput of the hot text utilities and of a full `scan()` (spec 07-testing).
//!
//! `cargo bench -p pulse-engine`

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::LazyLock;

use divan::counter::BytesCount;
use divan::{Bencher, black_box};
use pulse_core::Config;
use pulse_core::text::{self, tokens};
use pulse_engine::ScanOptions;

#[path = "support/gen_repo.rs"]
mod gen_repo;

fn main() {
    divan::main();
}

/// ~64 KB of mixed prose, code and a little non-ASCII, like a large instructions file.
static TEXT: LazyLock<String> = LazyLock::new(|| {
    let para = "## Conventions\n\nPrefer explicit error types over strings; keep functions \
                small. Résumé of the café rules: “quote” — dash.\n\n```rust\nfn main() { \
                println!(\"hi\"); }\n```\n\n- Run the formatter.\n- Review snapshots.\n\n";
    para.repeat(64 * 1024 / para.len())
});

#[divan::bench]
fn estimator(bencher: Bencher) {
    bencher
        .counter(BytesCount::of_str(&*TEXT))
        .bench(|| tokens::estimate(black_box(&TEXT), 1.0));
}

#[divan::bench]
fn normalize(bencher: Bencher) {
    bencher
        .counter(BytesCount::of_str(&*TEXT))
        .bench(|| text::normalize(black_box(&TEXT)));
}

/// The `large` fixture: 1,000 assets among 10,000 files, synthesized once per run.
static LARGE: LazyLock<tempfile::TempDir> = LazyLock::new(|| {
    let dir = tempfile::tempdir().unwrap();
    gen_repo::generate(dir.path(), 1_000, 10_000).unwrap();
    dir
});

#[divan::bench(sample_count = 30)]
fn scan_large(bencher: Bencher) {
    let root = LARGE.path();
    let config = Config::default();
    let opts = ScanOptions::default();
    bencher.bench(|| {
        let report = pulse_engine::scan(root, &config, &opts).unwrap();
        assert_eq!(report.assets_meta.len(), 1_000);
        report
    });
}
