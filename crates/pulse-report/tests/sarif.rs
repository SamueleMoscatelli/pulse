//! SARIF report snapshots, validated against the vendored SARIF 2.1.0 schema.

#![allow(clippy::unwrap_used)] // helpers outside #[test] fns

use std::path::{Path, PathBuf};

use pulse_core::{Config, Severity};
use pulse_engine::{ScanOptions, scan};
use pulse_report::{SarifOptions, render_sarif};
use serde_json::Value;

fn repo(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/repos")
        .join(name)
}

fn render(name: &str) -> String {
    let root = repo(name);
    let config = Config::load(&root, None).unwrap();
    let report = scan(&root, &config, &ScanOptions::default()).unwrap();
    let opts = SarifOptions {
        root: &root,
        min_severity: Severity::Info,
    };
    let mut out = Vec::new();
    render_sarif(&mut out, &report, &opts).unwrap();
    String::from_utf8(out).unwrap()
}

fn assert_valid(sarif: &str) {
    let schema: Value =
        serde_json::from_str(include_str!("schemas/sarif-schema-2.1.0.json")).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let instance: Value = serde_json::from_str(sarif).unwrap();
    let errors: Vec<String> = validator
        .iter_errors(&instance)
        .map(|e| format!("{} at {}", e, e.instance_path()))
        .collect();
    assert!(errors.is_empty(), "{errors:#?}");
}

#[test]
fn sarif_clean_repo_is_valid_and_empty() {
    let out = render("clean");
    assert_valid(&out);
    insta::assert_snapshot!(out);
}

#[test]
fn sarif_mixed_repo_is_valid() {
    let out = render("mixed");
    assert_valid(&out);
    insta::assert_snapshot!(out);
}

#[test]
fn sarif_schema_rejects_an_invalid_log() {
    let schema: Value =
        serde_json::from_str(include_str!("schemas/sarif-schema-2.1.0.json")).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let bad =
        serde_json::json!({ "version": "2.1.0", "runs": [{ "results": [{ "level": "fatal" }] }] });
    assert!(!validator.is_valid(&bad));
}

#[test]
fn sarif_rules_cover_results_with_security_scores() {
    let log: Value = serde_json::from_str(&render("mixed")).unwrap();
    let run = &log["runs"][0];
    let rules = run["tool"]["driver"]["rules"].as_array().unwrap();
    for r in run["results"].as_array().unwrap() {
        let i = usize::try_from(r["ruleIndex"].as_u64().unwrap()).unwrap();
        assert_eq!(rules[i]["id"], r["ruleId"]);
        assert_eq!(
            r["partialFingerprints"]["pulse/v1"].as_str().unwrap().len(),
            16
        );
    }
    for r in rules {
        let security = r["properties"]["tags"][0] == "security";
        assert_eq!(
            security,
            r["properties"]["security-severity"].is_string(),
            "{}",
            r["id"]
        );
        assert!(r["help"]["markdown"].as_str().unwrap().starts_with("# "));
    }
}
