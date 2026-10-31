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

#[test]
fn json_context_load_by_assistant() {
    let by = |name: &str| {
        let v: serde_json::Value = serde_json::from_str(&render(name, Severity::Info)).unwrap();
        v["metrics"]["context_load_by_assistant"].clone()
    };
    // One assistant: still listed. Nothing always-on: empty.
    let clean = by("clean");
    assert_eq!(clean.as_array().unwrap().len(), 2, "{clean}");
    assert_eq!(by("ondemand"), serde_json::json!([]));
    let mixed: serde_json::Value = serde_json::from_str(&render("mixed", Severity::Info)).unwrap();
    let m = &mixed["metrics"];
    let list = m["context_load_by_assistant"].as_array().unwrap();
    assert_eq!(list.len(), 8, "{list:?}");
    // Largest first; the top-level numbers are the largest assistant's.
    assert_eq!(list[0]["assistant"], "copilot");
    assert_eq!(list[0]["always_on_tokens"], m["always_on_tokens"]);
    assert_eq!(list[0]["context_load"], m["context_load"]);
    let tokens: Vec<u64> = list
        .iter()
        .map(|e| e["always_on_tokens"].as_u64().unwrap())
        .collect();
    assert!(tokens.windows(2).all(|w| w[0] >= w[1]), "{tokens:?}");
}

#[test]
fn json_scope_only_with_an_assistant_selection() {
    let all: serde_json::Value = serde_json::from_str(&render("mixed", Severity::Info)).unwrap();
    assert!(all.get("scope").is_none());

    let root = repo("mixed");
    let config = Config::load(&root, None).unwrap();
    let opts = ScanOptions {
        assistants: Some([pulse_core::Assistant::Copilot].into_iter().collect()),
        ..ScanOptions::default()
    };
    let report = scan(&root, &config, &opts).unwrap();
    let opts = JsonOptions {
        root: &root,
        min_severity: Severity::Info,
        generated_at: UNIX_EPOCH,
    };
    let mut out = Vec::new();
    render_json(&mut out, &report, &opts).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    let scope = &v["scope"];
    assert_eq!(scope["assistants"], serde_json::json!(["copilot"]));
    let style = scope["not_loaded"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["path"] == ".cursor/rules/style.mdc")
        .unwrap();
    assert_eq!(
        style,
        &serde_json::json!({
            "path": ".cursor/rules/style.mdc",
            "kind": "scoped_instructions",
            "reason": "other_assistants",
            "loaded_by": ["cursor"]
        })
    );
}
