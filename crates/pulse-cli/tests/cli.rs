//! The `pulse` binary end to end: exit codes, `init`, `scan`/`ci`, output modes, `rules`
//! and `explain`.

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

/// A CLAUDE.md line with a zero-width space (AI-SEC-001, critical), generated in code.
fn zwsp_line() -> String {
    format!("Always run{}the tests.\n", '\u{200B}')
}

#[test]
fn baseline_then_new_only_hides_accepted_findings() {
    let dir = tempfile::tempdir().unwrap();
    let d = tempdir_str(&dir);
    let md = dir.path().join("CLAUDE.md");
    std::fs::write(&md, format!("# Project\n\n{}", zwsp_line())).unwrap();
    // Without a baseline every finding is new, with a warning.
    let out = pulse(&["ci", "-C", &d, "--new-only"]).code(1);
    let stderr = String::from_utf8(out.get_output().stderr.clone()).unwrap();
    assert!(stderr.contains("no baseline"), "{stderr}");

    let out = pulse(&["baseline", "-C", &d]).code(0);
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("fingerprints to"), "{stdout}");
    let file = std::fs::read_to_string(dir.path().join(".pulse/baseline.json")).unwrap();
    assert!(file.starts_with("{\n  \"version\": 1,"), "{file}");

    // Shifted lines keep the old finding baselined.
    std::fs::write(
        &md,
        format!("# Project\n\nBuild with cargo.\n\n{}", zwsp_line()),
    )
    .unwrap();
    let out = pulse(&["ci", "-C", &d, "--new-only"]).code(0);
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(!stdout.contains("AI-SEC-001"), "{stdout}");
    assert!(stdout.contains("New code"), "{stdout}");
    // JSON still lists it without --new-only, marked as not new.
    let out = pulse(&["scan", "-C", &d, "-f", "json"]).code(0);
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("\"new\": false"), "{stdout}");

    // A new occurrence fails the gate again.
    std::fs::write(&md, format!("# Project\n\n{0}{0}", zwsp_line())).unwrap();
    pulse(&["ci", "-C", &d, "--new-only"]).code(1);
}

#[test]
fn bad_baseline_exits_two() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join(".pulse")).unwrap();
    std::fs::write(
        dir.path().join(".pulse/baseline.json"),
        r#"{"version":9,"fingerprints":[]}"#,
    )
    .unwrap();
    let out = pulse(&["scan", "-C", &tempdir_str(&dir)]).code(2);
    let stderr = String::from_utf8(out.get_output().stderr.clone()).unwrap();
    assert!(
        stderr.contains("unsupported baseline version 9"),
        "{stderr}"
    );
}

fn git(dir: &Path, args: &[&str]) {
    let ok = std::process::Command::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap()
        .status
        .success();
    assert!(ok, "git {args:?}");
}

#[test]
fn changed_since_scans_changed_and_untracked_files_only() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let d = tempdir_str(&dir);
    let bad = format!("# Project\n\n{}", zwsp_line());
    std::fs::write(root.join("CLAUDE.md"), &bad).unwrap();
    std::fs::write(root.join("AGENTS.md"), "# Agents\n\nUse cargo to build.\n").unwrap();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "base"]);

    // Nothing changed: the old finding in CLAUDE.md is out of scope.
    let out = pulse(&["ci", "-C", &d, "--changed-since", "HEAD"]).code(0);
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(!stdout.contains("AI-SEC-001"), "{stdout}");

    // A committed change and an untracked file are both in scope.
    std::fs::write(root.join("AGENTS.md"), &bad).unwrap();
    git(root, &["commit", "-q", "-am", "change"]);
    std::fs::create_dir_all(root.join(".github")).unwrap();
    std::fs::write(root.join(".github/copilot-instructions.md"), &bad).unwrap();
    let out = pulse(&["scan", "-C", &d, "-q", "--changed-since", "HEAD~1"]).code(0);
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("AGENTS.md:"), "{stdout}");
    assert!(
        stdout.contains(".github/copilot-instructions.md:"),
        "{stdout}"
    );
    assert!(!stdout.contains("CLAUDE.md:"), "{stdout}");

    // A bad ref is a usage error with git's message.
    let out = pulse(&["scan", "-C", &d, "--changed-since", "no-such-ref"]).code(2);
    let stderr = String::from_utf8(out.get_output().stderr.clone()).unwrap();
    assert!(stderr.contains("git diff"), "{stderr}");
}

#[test]
fn changed_since_outside_a_repo_exits_two() {
    let dir = tempfile::tempdir().unwrap();
    // Keep git from finding a repository above the tempdir.
    Command::cargo_bin("pulse")
        .unwrap()
        .env("GIT_CEILING_DIRECTORIES", dir.path().parent().unwrap())
        .args(["scan", "-C", &tempdir_str(&dir), "--changed-since", "main"])
        .assert()
        .code(2);
}

// `rules` and `explain`

fn stdout(out: &assert_cmd::assert::Assert) -> String {
    String::from_utf8(out.get_output().stdout.clone()).unwrap()
}

fn stderr(out: &assert_cmd::assert::Assert) -> String {
    String::from_utf8(out.get_output().stderr.clone()).unwrap()
}

#[test]
fn rules_table_shows_effective_severity_and_state() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("pulse.toml"),
        "[rules]\nAI-SEC-002 = \"off\"\nAI-SEC-013 = \"critical\"\n",
    )
    .unwrap();
    let out = pulse(&["rules", "-C", &tempdir_str(&dir), "--category", "SEC"]).code(0);
    insta::assert_snapshot!(stdout(&out));
}

#[test]
fn rules_json_lists_every_rule() {
    let dir = tempfile::tempdir().unwrap();
    let out = pulse(&["rules", "-C", &tempdir_str(&dir), "-f", "json"]).code(0);
    let rules: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(
        rules.as_array().unwrap().len(),
        pulse_rules::all_meta().count()
    );
    let out = pulse(&[
        "rules",
        "-C",
        &tempdir_str(&dir),
        "-f",
        "json",
        "--category",
        "efficiency",
    ])
    .code(0);
    insta::assert_snapshot!(stdout(&out));
}

/// The README rule table is `pulse rules -f markdown` under the default config.
/// `UPDATE_README=1 cargo test -p pulse-cli readme` regenerates it.
#[test]
fn readme_rule_table_is_in_sync() {
    const START: &str = "<!-- rules:start -->\n";
    const END: &str = "<!-- rules:end -->";
    let dir = tempfile::tempdir().unwrap();
    let out = pulse(&["rules", "-C", &tempdir_str(&dir), "-f", "markdown"]).code(0);
    let table = stdout(&out);
    assert_eq!(
        table.lines().filter(|l| l.starts_with("| [AI-")).count(),
        pulse_rules::all_meta().count()
    );

    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../README.md");
    let readme = std::fs::read_to_string(&path)
        .unwrap()
        .replace("\r\n", "\n");
    let start = readme
        .find(START)
        .expect("README lacks the rules:start marker")
        + START.len();
    let end = readme.find(END).expect("README lacks the rules:end marker");
    if std::env::var_os("UPDATE_README").is_some() {
        let updated = format!("{}{table}{}", &readme[..start], &readme[end..]);
        std::fs::write(&path, updated).unwrap();
    } else {
        assert!(
            readme[start..end] == table,
            "README rule table is stale: run `UPDATE_README=1 cargo test -p pulse-cli readme`"
        );
    }
}

#[test]
fn rules_with_bad_config_exits_two() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("pulse.toml"),
        "[rules]\nAI-SEC-02 = \"off\"\n",
    )
    .unwrap();
    let out = pulse(&["rules", "-C", &tempdir_str(&dir)]).code(2);
    assert!(
        stderr(&out).contains("did you mean `AI-SEC-002`?"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn explain_prints_the_embedded_docs() {
    let out = pulse(&["explain", "ai-sec-005", "--color", "never"]).code(0);
    insta::assert_snapshot!(stdout(&out));
    let colored = pulse(&["explain", "AI-SEC-005", "--color", "always"]).code(0);
    assert!(stdout(&colored).starts_with("\u{1b}[1m# AI-SEC-005"));
}

#[test]
fn explain_unknown_rule_exits_two_with_a_suggestion() {
    let out = pulse(&["explain", "AI-SEC-0011"]).code(2);
    assert!(stdout(&out).is_empty());
    insta::assert_snapshot!(stderr(&out));
}

const CUSTOM_RULE: &str = r#"
[[custom_rules]]
id = "X-ORG-001"
name = "No internal hostnames"
severity = "high"
pattern = '\binternal\.corp\b'
message = "Internal hostname leaked into AI context"
"#;

#[test]
fn custom_rule_finding_shows_in_terminal_and_rules() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("pulse.toml"), CUSTOM_RULE).unwrap();
    std::fs::write(
        dir.path().join("CLAUDE.md"),
        "# Rules\n\nDeploy to build.internal.corp only.\n",
    )
    .unwrap();
    let d = tempdir_str(&dir);
    let out = pulse(&["scan", "-C", &d, "--color", "never"]).code(0);
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(
        stdout.contains("X-ORG-001") && stdout.contains("Internal hostname leaked into AI context"),
        "{stdout}"
    );
    let out = pulse(&["rules", "-C", &d]).code(0);
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("X-ORG-001   high      yes"), "{stdout}");
    let out = pulse(&["rules", "-C", &d, "--category", "sec"]).code(0);
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(!stdout.contains("X-ORG-001"), "{stdout}");
}

#[test]
fn custom_rule_bad_regex_exits_two_naming_the_rule() {
    let dir = tempfile::tempdir().unwrap();
    let bad = CUSTOM_RULE.replace(r"'\binternal\.corp\b'", "'(oops'");
    std::fs::write(dir.path().join("pulse.toml"), bad).unwrap();
    let out = pulse(&["scan", "-C", &tempdir_str(&dir)]).code(2);
    let stderr = String::from_utf8(out.get_output().stderr.clone()).unwrap();
    assert!(
        stderr.contains("custom rule `X-ORG-001`: invalid `pattern`"),
        "{stderr}"
    );
}

#[test]
fn scan_assistant_lists_what_it_skips() {
    let out = pulse(&[
        "scan",
        "-v",
        "-C",
        &root("mixed"),
        "--assistant",
        "claude-code",
    ])
    .code(0);
    let text = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    assert!(
        text.contains("SCOPE          Claude Code only (--assistant)"),
        "{text}"
    );
    assert!(text.contains(".cursor/rules/style.mdc"), "{text}");
    let both = pulse(&[
        "scan",
        "-q",
        "-f",
        "markdown",
        "-C",
        &root("mixed"),
        "--assistant",
        "copilot,cursor",
    ])
    .code(0);
    let md = String::from_utf8_lossy(&both.get_output().stdout).into_owned();
    assert!(
        md.contains("- Scope: Copilot and Cursor only (--assistant)"),
        "{md}"
    );
    let bad = pulse(&["scan", "-C", &root("mixed"), "--assistant", "claude"]).code(2);
    let err = String::from_utf8_lossy(&bad.get_output().stderr).into_owned();
    assert!(
        err.contains("expected one of copilot, claude-code"),
        "{err}"
    );
}
