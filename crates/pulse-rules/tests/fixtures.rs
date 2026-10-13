//! Rule fixture harness (spec 07-testing → Rule fixture harness).
//!
//! Every `tests/fixtures/<RULE-ID>/` directory runs only that rule: `fail_*` files must
//! produce ≥ 1 finding (exactly the `line:col` set of a sibling `.expect`, when present),
//! `pass_*` files none. Every registered rule needs fixtures and `docs/rules/<ID>.md`.

#![allow(clippy::unwrap_used)] // helpers outside #[test] fns

use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use pulse_core::{Asset, AssetKind, Finding, LoadMode, OptionsStore, RuleCtx, Severity};
use pulse_rules::{PROJECT_RULES, RULES, all_meta};

fn crate_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn fixtures_dir() -> PathBuf {
    crate_dir().join("tests/fixtures")
}

/// Kind from `_kind`, else inferred from the file name.
fn kind_of(dir: &Path, name: &str) -> AssetKind {
    if let Ok(k) = fs::read_to_string(dir.join("_kind")) {
        return k.trim().parse().unwrap();
    }
    if name.ends_with("SKILL.md") {
        AssetKind::Skill
    } else if name.ends_with(".mdc") {
        AssetKind::ScopedInstructions
    } else if name.ends_with(".json") {
        AssetKind::McpConfig
    } else {
        AssetKind::Instructions
    }
}

fn asset(rel: &str, kind: AssetKind, bytes: Vec<u8>) -> Asset {
    Asset::from_bytes(PathBuf::from(rel), rel, kind, kind.default_load(), bytes)
}

/// Runs only `rule_id` (asset or project scope) over `assets`.
fn run(rule_id: &str, assets: &[Asset]) -> Vec<Finding> {
    let options = OptionsStore::empty();
    let files: HashSet<String> = assets.iter().map(|a| a.rel_path.clone()).collect();
    let ctx = RuleCtx {
        options: &options,
        root: Path::new("."),
        files: &files,
    };
    let mut out = Vec::new();
    if let Some(rule) = RULES.iter().find(|r| r.meta().id == rule_id) {
        for a in assets.iter().filter(|a| rule.meta().kinds.contains(a.kind)) {
            rule.check(a, &ctx, &mut out);
        }
    }
    if let Some(rule) = PROJECT_RULES.iter().find(|r| r.meta().id == rule_id) {
        rule.check_project(assets, &ctx, &mut out);
    }
    out.retain(|f| f.rule_id == rule_id);
    out
}

fn is_fixture(name: &str) -> bool {
    (name.starts_with("fail_") || name.starts_with("pass_")) && !name.ends_with(".expect")
}

#[test]
fn fixtures() {
    let mut failures = Vec::new();
    let mut dirs: Vec<_> = fs::read_dir(fixtures_dir())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    for dir in dirs {
        let rule_id = dir.file_name().unwrap().to_str().unwrap().to_owned();
        let mut names: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .filter(|n| is_fixture(n))
            .collect();
        names.sort();
        let assets: Vec<Asset> = names
            .iter()
            .map(|n| asset(n, kind_of(&dir, n), fs::read(dir.join(n)).unwrap()))
            .collect();
        let findings = run(&rule_id, &assets);
        for name in &names {
            let actual: BTreeSet<String> = findings
                .iter()
                .filter(|f| f.location.path.as_ref() == Path::new(name))
                .map(|f| format!("{}:{}", f.location.line, f.location.col))
                .collect();
            let fixture = format!("{rule_id}/{name}");
            if name.starts_with("pass_") {
                if !actual.is_empty() {
                    failures.push(format!("{fixture}: expected none, got {actual:?}"));
                }
            } else if let Ok(expect) = fs::read_to_string(dir.join(format!("{name}.expect"))) {
                let expected: BTreeSet<String> = expect
                    .lines()
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                    .map(str::to_owned)
                    .collect();
                if expected != actual {
                    failures.push(format!("{fixture}: expected {expected:?}, got {actual:?}"));
                }
            } else if actual.is_empty() {
                failures.push(format!("{fixture}: expected >= 1 finding, got none"));
            }
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn every_rule_has_fixtures_and_docs() {
    let docs = crate_dir().join("../../docs/rules");
    let mut missing = Vec::new();
    for meta in all_meta() {
        let names: Vec<String> = fs::read_dir(fixtures_dir().join(meta.id))
            .map(|rd| {
                rd.filter_map(|e| e.ok()?.file_name().into_string().ok())
                    .collect()
            })
            .unwrap_or_default();
        for prefix in ["fail_", "pass_"] {
            if !names.iter().any(|n| n.starts_with(prefix) && is_fixture(n)) {
                missing.push(format!("{}: no {prefix}* fixture", meta.id));
            }
        }
        if !docs.join(format!("{}.md", meta.id)).is_file() {
            missing.push(format!("{}: no docs/rules/{}.md", meta.id, meta.id));
        }
    }
    assert!(missing.is_empty(), "\n{}", missing.join("\n"));
}

// Generated cases: content that must not be committed raw.

fn str002(kind: AssetKind, rel: &str, bytes: &[u8]) -> Vec<Finding> {
    run("AI-STR-002", &[asset(rel, kind, bytes.to_vec())])
}

#[test]
fn str002_invalid_utf8_is_located() {
    let f = str002(
        AssetKind::Instructions,
        "CLAUDE.md",
        b"# Rules\nUse \xff\xfe tabs\n",
    );
    assert_eq!(f.len(), 1);
    assert_eq!((f[0].location.line, f[0].location.col), (2, 5));
    assert!(f[0].message.starts_with("Invalid UTF-8"));
    assert!(
        str002(
            AssetKind::Instructions,
            "CLAUDE.md",
            "ok \u{FFFD}".as_bytes()
        )
        .is_empty()
    );
}

#[test]
fn str002_skipped_file_is_info() {
    let a = Asset::skipped(
        PathBuf::from("big.md"),
        "big.md",
        AssetKind::Instructions,
        LoadMode::Always,
        "over scan.max_file_bytes (1048576 bytes)",
    );
    let f = run("AI-STR-002", &[a]);
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].severity, Severity::Info);
    assert!(
        f[0].message
            .starts_with("File skipped: over scan.max_file_bytes"),
        "{}",
        f[0].message
    );
}

#[test]
fn str002_settings_and_tools_shapes() {
    let bad = str002(
        AssetKind::Settings,
        ".claude/settings.json",
        br#"{"permissions": {"allow": "Bash"}}"#,
    );
    assert_eq!(bad.len(), 1, "{bad:?}");
    assert!(bad[0].message.starts_with("Invalid shape"));
    assert!(
        str002(
            AssetKind::Settings,
            ".claude/settings.json",
            br#"{"permissions": {"allow": ["Bash(ls)"]}}"#
        )
        .is_empty()
    );
    assert_eq!(str002(AssetKind::Tools, "a.tools.json", b"[").len(), 1);
    assert!(str002(AssetKind::Custom, "x.txt", b"{ not json").is_empty());
}
