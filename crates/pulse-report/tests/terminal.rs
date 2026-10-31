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
    render_width(name, quiet, verbose, None)
}

fn render_width(name: &str, quiet: bool, verbose: bool, width: Option<usize>) -> String {
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
        width,
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

const FIX_HINT: &str = "Fixes shown for medium and higher · run pulse scan -v to see all";
const FIX_FOOTER: &str = "↳ Fixes shown for medium and higher findings. Run with -v";

#[test]
fn terminal_hidden_fixes_are_announced() {
    insta::assert_snapshot!(render("advice", false, false));
}

#[test]
fn terminal_fix_hint_absent_with_verbose_quiet_or_nothing_hidden() {
    for out in [
        render("advice", false, true),
        render("advice", true, false),
        render("mixed", false, false),
    ] {
        assert!(!out.contains(FIX_HINT), "{out}");
        assert!(!out.contains(FIX_FOOTER), "{out}");
    }
}

#[test]
fn terminal_nothing_always_on() {
    insta::assert_snapshot!(render("ondemand", false, false));
}

#[test]
fn terminal_one_assistant() {
    insta::assert_snapshot!(render("single", false, false));
}

#[test]
fn terminal_narrow_width_wraps_under_the_text() {
    let out = render_width("advice", false, true, Some(72));
    insta::assert_snapshot!(out);
    assert!(out.lines().all(|l| l.chars().count() <= 72), "{out}");
}

#[test]
fn terminal_quiet_ignores_width() {
    assert_eq!(
        render_width("mixed", true, false, Some(40)),
        render("mixed", true, false)
    );
}

#[test]
fn terminal_assistant_scope_lists_skipped_files_with_verbose() {
    let root = repo("mixed");
    let config = Config::load(&root, None).unwrap();
    let opts = ScanOptions {
        assistants: Some([pulse_core::Assistant::Copilot].into_iter().collect()),
        ..ScanOptions::default()
    };
    let report = scan(&root, &config, &opts).unwrap();
    let out = |verbose| {
        let opts = TerminalOptions {
            root: &root,
            color: false,
            quiet: false,
            verbose,
            min_severity: Severity::Info,
            width: None,
        };
        let mut out = Vec::new();
        render_terminal(&mut out, &report, &opts).unwrap();
        String::from_utf8(out).unwrap()
    };
    let brief = out(false);
    let scope: String = brief
        .lines()
        .skip_while(|l| !l.starts_with(" SCOPE"))
        .take(2)
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(scope);
    assert!(out(true).contains("   .cursor/rules/style.mdc"));
}
