//! Markdown report snapshots over fixture repos (fixed timings).

#![allow(clippy::unwrap_used)] // helpers outside #[test] fns

use std::path::{Path, PathBuf};

use pulse_core::{Config, Severity};
use pulse_engine::{Report, ScanOptions, scan};
use pulse_report::{MarkdownOptions, render_markdown};

fn repo(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/repos")
        .join(name)
}

fn report(root: &Path) -> Report {
    let config = Config::load(root, None).unwrap();
    scan(root, &config, &ScanOptions::default()).unwrap()
}

fn render(root: &Path, report: &Report, min_severity: Severity) -> String {
    let opts = MarkdownOptions { root, min_severity };
    let mut out = Vec::new();
    render_markdown(&mut out, report, &opts).unwrap();
    String::from_utf8(out).unwrap()
}

#[test]
fn markdown_clean_repo_passes() {
    let root = repo("clean");
    insta::assert_snapshot!(render(&root, &report(&root), Severity::Info));
}

#[test]
fn markdown_mixed_repo_fails() {
    let root = repo("mixed");
    insta::assert_snapshot!(render(&root, &report(&root), Severity::Info));
}

#[test]
fn markdown_caps_findings_at_fifty() {
    let root = repo("mixed");
    let mut report = report(&root);
    let template = report.findings[0].clone();
    report.findings = (0..60)
        .map(|i| {
            let mut f = template.clone();
            f.location.line = i + 1;
            f
        })
        .collect();
    let out = render(&root, &report, Severity::Info);
    let rows = out.lines().filter(|l| l.contains("AI-COG-006")).count();
    assert_eq!(rows, 50);
    assert!(out.contains("\n+10 more\n"), "{out}");
}

#[test]
fn markdown_nothing_always_on() {
    let root = repo("ondemand");
    insta::assert_snapshot!(render(&root, &report(&root), Severity::Info));
}

#[test]
fn markdown_one_assistant() {
    let root = repo("single");
    insta::assert_snapshot!(render(&root, &report(&root), Severity::Info));
}
