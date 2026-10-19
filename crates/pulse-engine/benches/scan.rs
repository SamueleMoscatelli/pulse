//! Throughput of the hot text utilities and of a full `scan()` (spec 07-testing).
//!
//! `cargo bench -p pulse-engine`

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use divan::counter::BytesCount;
use divan::{Bencher, black_box};
use pulse_core::text::{self, tokens};
use pulse_core::{Asset, AssetKind, Config, ImportGraph, OptionsStore, RuleCtx};
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

/// 1,000 instruction files × 20 distinct ~40-word paragraphs, views already parsed (as
/// they are once the asset rules have run).
static PARAGRAPHS: LazyLock<Vec<Asset>> = LazyLock::new(|| {
    const SYLLABLES: [&str; 16] = [
        "ka", "lo", "mi", "ne", "ru", "sa", "to", "vi", "be", "da", "fo", "gu", "ha", "je", "pe",
        "zo",
    ];
    let mut seed = 0x2545_F491_4F6C_DD1Du64;
    let mut word = |out: &mut String| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        for i in 0..3 {
            out.push_str(SYLLABLES[((seed >> (i * 4)) & 15) as usize]);
        }
    };
    (0..1_000)
        .map(|a| {
            let mut text = format!("# Package {a}\n\n");
            for _ in 0..20 {
                for w in 0..40 {
                    word(&mut text);
                    text.push_str(if w == 39 { ".\n\n" } else { " " });
                }
            }
            let rel = format!("packages/p{a:04}/CLAUDE.md");
            let asset = Asset::from_bytes(
                PathBuf::from(&rel),
                &rel,
                AssetKind::Instructions,
                AssetKind::Instructions.default_load(),
                text.into_bytes(),
            );
            asset.markdown();
            asset.normalized();
            asset
        })
        .collect()
});

/// AI-EFF-003 over [`PARAGRAPHS`] (target < 10 ms, spec AI-EFF-003 / roadmap P5.1).
#[divan::bench(sample_count = 30)]
fn duplicated_content(bencher: Bencher) {
    let rule = pulse_rules::PROJECT_RULES
        .iter()
        .find(|r| r.meta().id == "AI-EFF-003")
        .unwrap();
    let config = Config::default();
    let options = OptionsStore::empty();
    let files = HashSet::new();
    let imports = ImportGraph::default();
    let ctx = RuleCtx {
        config: &config,
        options: &options,
        root: Path::new("."),
        files: &files,
        imports: &imports,
    };
    let assets = &*PARAGRAPHS;
    bencher.bench(|| {
        let mut out = Vec::new();
        rule.check_project(black_box(assets), &ctx, &mut out);
        assert!(out.is_empty());
        out
    });
}
