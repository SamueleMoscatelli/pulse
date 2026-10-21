//! The `pulse` binary end to end: exit codes, `init`, `scan`/`ci` and output modes.

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
        .env_remove("GITHUB_STEP_SUMMARY")
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
fn markdown_format_renders_a_pr_comment() {
    let out = pulse(&["scan", "-C", &root("mixed"), "-f", "markdown"]).code(0);
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(
        stdout.starts_with("## Pulse Quality Gate: ❌ FAILED"),
        "{stdout}"
    );
    assert!(stdout.contains("<details><summary><code>tools/fetch.json</code>"));
}

#[test]
fn json_and_sarif_formats_write_their_schema() {
    let out = pulse(&["ci", "-C", &root("mixed"), "-f", "json"]).code(1);
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(
        stdout.starts_with("{\n  \"schema\": \"pulse/report@1\""),
        "{stdout}"
    );

    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("pulse.sarif");
    let f = file.to_string_lossy().into_owned();
    pulse(&["scan", "-C", &root("mixed"), "-f", "sarif", "-o", &f])
        .code(0)
        .stdout("");
    let sarif = std::fs::read_to_string(&file).unwrap();
    assert!(sarif.contains("\"version\": \"2.1.0\""), "{sarif}");
    assert!(sarif.contains("\"ruleId\": \"AI-STR-002\""), "{sarif}");
}

#[test]
fn ci_on_github_actions_annotates_and_appends_step_summary() {
    let dir = tempfile::tempdir().unwrap();
    let summary = dir.path().join("summary.md");
    std::fs::write(&summary, "previous step\n").unwrap();
    let out = Command::cargo_bin("pulse")
        .unwrap()
        .env("GITHUB_ACTIONS", "true")
        .env("GITHUB_STEP_SUMMARY", &summary)
        .args(["ci", "-C", &root("mixed")])
        .assert()
        .code(1);
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(
        stdout.contains("::error file=tools/fetch.json,line=1,col=1,title=AI-STR-002::"),
        "{stdout}"
    );
    assert!(stdout.contains("Pulse Quality Gate: FAILED"), "{stdout}");
    let md = std::fs::read_to_string(&summary).unwrap();
    assert!(
        md.starts_with("previous step\n## Pulse Quality Gate: ❌ FAILED"),
        "{md}"
    );
}

#[test]
fn github_format_without_step_summary_writes_no_file() {
    let out = Command::cargo_bin("pulse")
        .unwrap()
        .env_remove("GITHUB_ACTIONS")
        .env_remove("GITHUB_STEP_SUMMARY")
        .args(["scan", "-C", &root("clean"), "-f", "github"])
        .assert()
        .code(0);
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(stdout.starts_with("Pulse Quality Gate: PASSED"), "{stdout}");
}

#[test]
fn quiet_min_severity_and_skip_rule_filter_output() {
    let out = pulse(&["-q", "scan", "-C", &root("mixed")]).code(0);
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert_eq!(
        stdout,
        concat!(
            ".claude/agents/planner.md:3:1: MEDIUM AI-COG-006 Weak routing description \"Plans work.\": 2 words (< 20), no trigger cue\n",
            ".claude/skills/review/SKILL.md:3:1: MEDIUM AI-COG-006 Weak routing description \"Review code.\": 2 words (< 20), no trigger cue\n",
            ".github/agents/triage.agent.md:2:1: MEDIUM AI-COG-006 Weak routing description \"Triage.\": 1 word (< 20), no trigger cue\n",
            ".github/chatmodes/debug.chatmode.md:2:1: MEDIUM AI-COG-006 Weak routing description \"Debug.\": 1 word (< 20), no trigger cue\n",
            "tools/fetch.json:1:1: HIGH AI-STR-002 Invalid shape: `tools` must be an array, found object\n",
        )
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

fn tempdir_str(dir: &tempfile::TempDir) -> String {
    dir.path().to_string_lossy().into_owned()
}

#[test]
fn init_then_scan_uses_the_generated_config() {
    let dir = tempfile::tempdir().unwrap();
    let d = tempdir_str(&dir);
    std::fs::write(
        dir.path().join("CLAUDE.md"),
        "# Project\n\nUse cargo to build.\n",
    )
    .unwrap();
    pulse(&["init", "-C", &d]).code(0);
    assert!(dir.path().join("pulse.toml").is_file());
    // A second `init` refuses to overwrite.
    pulse(&["init", "-C", &d]).code(2);
    let out = pulse(&["ci", "-C", &d]).code(0);
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("QUALITY GATE   PASSED"), "{stdout}");
}

#[test]
fn ci_fails_on_a_critical_finding() {
    let dir = tempfile::tempdir().unwrap();
    // Zero-width space (AI-SEC-001, critical), generated rather than committed raw.
    let text = format!("# Project\n\nAlways run{}the tests.\n", '\u{200B}');
    std::fs::write(dir.path().join("CLAUDE.md"), text).unwrap();
    let d = tempdir_str(&dir);
    let out = pulse(&["ci", "-C", &d]).code(1);
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("AI-SEC-001"), "{stdout}");
    assert!(stdout.contains("QUALITY GATE   FAILED"), "{stdout}");
    // `scan` reports the same finding but exits 0.
    pulse(&["scan", "-C", &d]).code(0);
}

#[test]
fn unknown_rule_in_config_exits_two() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("pulse.toml"),
        "[rules]\n\"AI-STR-02\" = \"off\"\n",
    )
    .unwrap();
    let out = pulse(&["scan", "-C", &tempdir_str(&dir)]).code(2);
    let stderr = String::from_utf8(out.get_output().stderr.clone()).unwrap();
    assert!(stderr.contains("unknown rule `AI-STR-02`"), "{stderr}");
}
