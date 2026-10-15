//! Terminal report snapshots over fixture repos (color off, fixed timings).

#![allow(clippy::unwrap_used)] // helpers outside #[test] fns

use std::path::{Path, PathBuf};
use std::time::Duration;

use pulse_core::{Config, Severity};
use pulse_engine::{ScanOptions, Timings, scan};
use pulse_report::{TerminalOptions, render_terminal};

fn repo(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/repos")
        .join(name)
}

fn render(name: &str, quiet: bool, verbose: bool) -> String {
    let root = repo(name);
    let config = Config::load(&root, None).unwrap();
    let mut report = scan(&root, &config, &ScanOptions::default()).unwrap();
    report.timings = Timings {
        total: Duration::from_millis(8),
        ..Timings::default()
    };
    let opts = TerminalOptions {
        root: &root,
        color: false,
        quiet,
        verbose,
        min_severity: Severity::Info,
        window: config.context.window,
    };
    let mut out = Vec::new();
    render_terminal(&mut out, &report, &opts).unwrap();
    String::from_utf8(out).unwrap()
}

#[test]
fn terminal_clean_repo_passes() {
    insta::assert_snapshot!(render("clean", false, false));
}

#[test]
fn terminal_mixed_repo_fails() {
    insta::assert_snapshot!(render("mixed", false, false));
}

#[test]
fn terminal_quiet_lists_findings_only() {
    insta::assert_snapshot!(render("mixed", true, false));
}

#[test]
fn terminal_verbose_shows_every_remediation() {
    let out = render("mixed", false, true);
    let findings = render("mixed", true, false).lines().count();
    assert_eq!(out.matches('↳').count(), findings);
}
