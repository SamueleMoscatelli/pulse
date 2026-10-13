//! End-to-end scans through the engine pipeline.

#![allow(clippy::unwrap_used)] // helpers outside #[test] fns

use std::fs;
use std::path::{Path, PathBuf};

use pulse_core::{Config, Severity};
use pulse_engine::{Report, ScanOptions, scan};

fn mixed() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/repos/mixed")
}

/// Everything in a report except timings, as comparable text.
fn render(r: &Report) -> String {
    let mut s = String::new();
    for f in &r.findings {
        s += &format!(
            "{} {}:{}:{}-{}:{} {} {} {:016x}\n",
            f.rule_id,
            f.location.path.display(),
            f.location.line,
            f.location.col,
            f.location.end_line,
            f.location.end_col,
            f.severity,
            f.message,
            f.fingerprint
        );
    }
    for a in &r.assets_meta {
        s += &format!("{a:?}\n");
    }
    s
}

#[test]
fn scan_mixed_is_deterministic() {
    let root = mixed();
    let config = Config::load(&root, None).unwrap();
    let first = render(&scan(&root, &config, &ScanOptions::default()).unwrap());
    assert!(first.contains("rel_path: \"CLAUDE.md\""), "{first}");
    for _ in 0..9 {
        let again = render(&scan(&root, &config, &ScanOptions::default()).unwrap());
        assert_eq!(first, again);
    }
}

fn write(dir: &Path, rel: &str, bytes: &[u8]) {
    let p = dir.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, bytes).unwrap();
}

#[test]
fn scan_reports_invalid_content_and_skipped_files() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "CLAUDE.md", b"# Rules\nUse \xff tabs.\n");
    write(root, ".mcp.json", br#"{ "mcpServers": [] }"#);
    write(root, "AGENTS.md", &[b'a'; 64]);
    write(
        root,
        ".claude/agents/ok.md",
        b"---\ndescription: fine\n---\nBody\n",
    );
    let mut config = Config::default();
    config.scan.max_file_bytes = 32;

    let report = scan(root, &config, &ScanOptions::default()).unwrap();
    let got: Vec<_> = report
        .findings
        .iter()
        .map(|f| {
            let rel = f.location.path.strip_prefix(root).unwrap();
            (
                rel.to_string_lossy().into_owned(),
                f.severity,
                f.fingerprint != 0,
            )
        })
        .collect();
    assert_eq!(
        got,
        [
            (".mcp.json".to_owned(), Severity::High, true),
            ("AGENTS.md".to_owned(), Severity::Info, true),
            ("CLAUDE.md".to_owned(), Severity::High, true),
        ]
    );
    assert_eq!(report.assets_meta.len(), 4);

    let only_claude = ScanOptions {
        paths: vec!["CLAUDE.md".into()],
        ..ScanOptions::default()
    };
    assert_eq!(scan(root, &config, &only_claude).unwrap().findings.len(), 1);
    let skip = ScanOptions {
        skip_rules: vec!["AI-STR".into()],
        ..ScanOptions::default()
    };
    assert!(scan(root, &config, &skip).unwrap().findings.is_empty());

    let low = Config::parse(
        "[scan]\nmax_file_bytes = 32\n[rules]\nAI-STR-002 = \"low\"\n",
        Path::new("pulse.toml"),
    )
    .unwrap();
    let report = scan(root, &low, &ScanOptions::default()).unwrap();
    assert!(report.findings.iter().all(|f| f.severity == Severity::Low));
}

#[test]
fn scan_rejects_unknown_rule_in_config() {
    let config = Config::parse("[rules]\nAI-STR-020 = \"off\"\n", Path::new("pulse.toml")).unwrap();
    let err = scan(&mixed(), &config, &ScanOptions::default()).unwrap_err();
    assert!(
        err.to_string().contains("did you mean `AI-STR-002`"),
        "{err}"
    );
}
