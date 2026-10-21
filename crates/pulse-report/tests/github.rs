//! GitHub workflow-command snapshots over fixture repos.

#![allow(clippy::unwrap_used)] // helpers outside #[test] fns

use std::path::{Path, PathBuf};

use pulse_core::{Config, Severity};
use pulse_engine::{ScanOptions, scan};
use pulse_report::{GithubOptions, render_github};

fn repo(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/repos")
        .join(name)
}

fn render(name: &str) -> String {
    let root = repo(name);
    let config = Config::load(&root, None).unwrap();
    let report = scan(&root, &config, &ScanOptions::default()).unwrap();
    let opts = GithubOptions {
        root: &root,
        min_severity: Severity::Info,
    };
    let mut out = Vec::new();
    render_github(&mut out, &report, &opts).unwrap();
    String::from_utf8(out).unwrap()
}

#[test]
fn github_clean_repo_passes() {
    insta::assert_snapshot!(render("clean"));
}

#[test]
fn github_mixed_repo_fails() {
    insta::assert_snapshot!(render("mixed"));
}
