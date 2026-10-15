//! `pulse scan` / `pulse ci` exit codes and output modes, end to end.

#![allow(clippy::unwrap_used)] // helpers outside #[test] fns

use std::path::{Path, PathBuf};

use assert_cmd::Command;

fn repo(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/repos")
        .join(name)
}

fn pulse(args: &[&str]) -> assert_cmd::assert::Assert {
    Command::cargo_bin("pulse")
        .unwrap()
        .env_remove("GITHUB_ACTIONS")
        .args(args)
        .assert()
}

fn root(name: &str) -> String {
    repo(name).to_string_lossy().into_owned()
}

#[test]
fn scan_exits_zero_even_when_gate_fails() {
    pulse(&["scan", "-C", &root("mixed")]).code(0);
}

#[test]
fn scan_fail_on_gate_exits_one() {
    pulse(&["scan", "-C", &root("mixed"), "--fail-on-gate"]).code(1);
}

#[test]
fn ci_exits_one_on_failed_gate_and_zero_on_passed() {
    pulse(&["ci", "-C", &root("mixed")]).code(1);
    pulse(&["ci", "-C", &root("clean")]).code(0);
}

#[test]
fn bad_config_exits_two() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("pulse.toml"), "[gate]\nnope = 1\n").unwrap();
    let d = dir.path().to_string_lossy().into_owned();
    pulse(&["scan", "-C", &d]).code(2);
}

#[test]
fn unimplemented_format_exits_two() {
    pulse(&["scan", "-C", &root("clean"), "-f", "json"]).code(2);
}

#[test]
fn github_format_falls_back_to_terminal() {
    let out = pulse(&["ci", "-C", &root("clean"), "-f", "github"]).code(0);
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("QUALITY GATE   PASSED"), "{stdout}");
}

#[test]
fn quiet_min_severity_and_skip_rule_filter_output() {
    let out = pulse(&["-q", "scan", "-C", &root("mixed")]).code(0);
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert_eq!(
        stdout,
        "tools/fetch.json:1:1: HIGH AI-STR-002 Invalid shape: `tools` must be an array, found object\n"
    );
    let out = pulse(&[
        "-q",
        "scan",
        "-C",
        &root("mixed"),
        "--min-severity",
        "critical",
    ])
    .code(0);
    assert!(out.get_output().stdout.is_empty());
    // Skipping the only failing rule passes the gate.
    pulse(&["ci", "-C", &root("mixed"), "--skip-rule", "AI-STR-002"]).code(0);
}

#[test]
fn color_always_emits_ansi_and_never_does_not() {
    let out = pulse(&["scan", "-C", &root("clean"), "--color", "always"]).code(0);
    assert!(out.get_output().stdout.contains(&0x1b));
    let out = pulse(&["scan", "-C", &root("clean"), "--color", "never"]).code(0);
    assert!(!out.get_output().stdout.contains(&0x1b));
}

#[test]
fn output_flag_writes_file_and_timings_go_to_stderr() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("report.txt");
    let out = pulse(&[
        "scan",
        "-C",
        &root("clean"),
        "--timings",
        "-o",
        &file.to_string_lossy(),
    ])
    .code(0);
    assert!(out.get_output().stdout.is_empty());
    let stderr = String::from_utf8(out.get_output().stderr.clone()).unwrap();
    assert!(stderr.starts_with("timings: discover "), "{stderr}");
    assert!(
        std::fs::read_to_string(&file)
            .unwrap()
            .contains("No issues found.")
    );
}
