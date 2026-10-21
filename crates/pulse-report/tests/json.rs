//! JSON report snapshots over fixture repos (fixed timings and timestamp).

#![allow(clippy::unwrap_used)] // helpers outside #[test] fns

use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use pulse_core::{Config, Severity};
use pulse_engine::{ScanOptions, Timings, scan};
use pulse_report::{JSON_SCHEMA, JsonOptions, render_json};

fn repo(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/repos")
        .join(name)
}

fn render(name: &str, min_severity: Severity) -> String {
    let root = repo(name);
    let config = Config::load(&root, None).unwrap();
    let mut report = scan(&root, &config, &ScanOptions::default()).unwrap();
    report.timings = Timings {
        total: Duration::from_millis(8),
        ..Timings::default()
    };
    let opts = JsonOptions {
        root: &root,
        min_severity,
        generated_at: UNIX_EPOCH + Duration::from_secs(1_791_799_200),
    };
    let mut out = Vec::new();
    render_json(&mut out, &report, &opts).unwrap();
    let root = root.to_string_lossy().replace('\\', "/");
    String::from_utf8(out).unwrap().replace(&root, "<root>")
}

#[test]
fn json_clean_repo_passes() {
    insta::assert_snapshot!(render("clean", Severity::Info));
}

#[test]
fn json_mixed_repo_fails() {
    insta::assert_snapshot!(render("mixed", Severity::Info));
}

#[test]
fn json_min_severity_filters_findings_not_metrics() {
    let all: serde_json::Value = serde_json::from_str(&render("mixed", Severity::Info)).unwrap();
    let high: serde_json::Value = serde_json::from_str(&render("mixed", Severity::High)).unwrap();
    assert_eq!(all["schema"], JSON_SCHEMA);
    assert_eq!(all["metrics"], high["metrics"]);
    let n = |v: &serde_json::Value| v["findings"].as_array().unwrap().len();
    assert!(n(&high) < n(&all));
    let severities = high["findings"].as_array().unwrap().iter();
    assert!(
        severities
            .map(|f| &f["severity"])
            .all(|s| s == "high" || s == "critical")
    );
}
