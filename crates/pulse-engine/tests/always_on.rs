//! AI-EFF-002 (always-on context over budget) end to end: the finding must survive the
//! pipeline (dedup used to drop it next to AI-EFF-001 on the same file), and the report must
//! not contradict itself: the finding and `metrics.always_on_tokens` agree on the budget.

#![allow(clippy::unwrap_used)] // helpers outside #[test] fns

use std::fs;
use std::path::{Path, PathBuf};

use pulse_core::Config;
use pulse_core::text::tokens;
use pulse_engine::{Report, ScanOptions, scan};

const RULE: &str = "AI-EFF-002";

fn write(dir: &Path, rel: &str, text: &str) {
    let p = dir.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

/// Prose (no code, no headings) estimated at exactly `target` tokens. Lines differ, so no
/// two are duplicates; `tag` keeps files apart too.
fn prose(tag: &str, target: u32) -> String {
    let cal = Config::default().context.calibration;
    let mut text = String::new();
    let mut i = 0;
    while tokens::estimate(&text, cal) + 40 < target {
        i += 1;
        text += &format!("Rule {tag}{i}: keep module {tag}{i} documented, tested and small.\n");
    }
    // Each extra letter adds well under one token, so the exact count is reached.
    while tokens::estimate(&text, cal) < target {
        text.push('a');
    }
    assert_eq!(tokens::estimate(&text, cal), target, "{tag}");
    text
}

fn run(root: &Path) -> (Report, Config) {
    let config = Config::load(root, None).unwrap();
    let report = scan(root, &config, &ScanOptions::default()).unwrap();
    (report, config)
}

fn rel(report_root: &Path, f: &pulse_core::Finding) -> String {
    let p = f.location.path.strip_prefix(report_root).unwrap();
    p.to_string_lossy().replace('\\', "/")
}

fn always_on(report: &Report) -> Vec<&pulse_core::Finding> {
    report
        .findings
        .iter()
        .filter(|f| f.rule_id == RULE)
        .collect()
}

/// The total in an AI-EFF-002 message (`Always-on context[ for X] <total> tokens > …`).
fn message_total(message: &str) -> u64 {
    let head = message.split(" tokens > ").next().unwrap();
    head.rsplit(' ').next().unwrap().parse().unwrap()
}

/// `AGENTS.md` (Copilot, Cursor, Windsurf, Codex, Kiro) plus five always-applied Cursor rules:
/// only Cursor goes over the budget; Claude Code loads none of it.
fn one_assistant_over(root: &Path) {
    write(root, "AGENTS.md", &prose("agents", 5000));
    for n in 1..=5 {
        let body = prose(&format!("cursor{n}x"), 800 - 10 * n);
        write(
            root,
            &format!(".cursor/rules/r{n}.mdc"),
            &format!("---\nalwaysApply: true\n---\n{body}"),
        );
    }
}

/// Over the budget for every assistant; Copilot loads the most (both root files and the
/// `applyTo: "**"` instructions).
fn every_assistant_over(root: &Path) {
    write(root, "AGENTS.md", &prose("agents", 9500));
    write(root, "CLAUDE.md", &prose("claude", 9000));
    for n in 1..=4 {
        let body = prose(&format!("gh{n}x"), 600 - 10 * n);
        write(
            root,
            &format!(".github/instructions/i{n}.instructions.md"),
            &format!("---\napplyTo: \"**\"\n---\n{body}"),
        );
    }
}

#[test]
fn one_assistant_over_the_budget_is_reported_once() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    one_assistant_over(root);
    let (report, _) = run(root);

    let by: Vec<_> = report
        .context
        .by_assistant
        .iter()
        .map(|&(a, t)| (a.name(), t))
        .collect();
    assert_eq!(by[0].0, "Cursor", "{by:?}");
    assert!(by[0].1 > 8000 && by[1].1 <= 8000, "{by:?}");

    let found = always_on(&report);
    assert_eq!(found.len(), 1, "{:#?}", report.findings);
    let f = found[0];
    assert_eq!(
        rel(root, f),
        "AGENTS.md",
        "points at the largest contributor"
    );
    assert_eq!((f.location.line, f.location.col), (1, 1));
    let m = &f.message;
    assert!(m.starts_with("Always-on context for Cursor "), "{m}");
    assert_eq!(message_total(m), report.metrics.always_on_tokens, "{m}");
    // Top 5 of 6 contributors, largest first.
    let top = m.split("; top: ").nth(1).unwrap();
    let names: Vec<_> = top
        .split(", ")
        .map(|e| e.split(' ').next().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "AGENTS.md",
            ".cursor/rules/r1.mdc",
            ".cursor/rules/r2.mdc",
            ".cursor/rules/r3.mdc",
            ".cursor/rules/r4.mdc",
            "…"
        ],
        "{m}"
    );
    // AI-EFF-001 on the same file is reported too: they are different problems.
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.rule_id == "AI-EFF-001" && rel(root, f) == "AGENTS.md")
    );
}

#[test]
fn every_assistant_over_the_budget_is_reported_once() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    every_assistant_over(root);
    let (report, _) = run(root);

    assert!(
        report.context.by_assistant.iter().all(|&(_, t)| t > 8000),
        "{:?}",
        report.context.by_assistant
    );
    let found = always_on(&report);
    assert_eq!(found.len(), 1, "{:#?}", report.findings);
    let f = found[0];
    assert_eq!(
        rel(root, f),
        "AGENTS.md",
        "points at the largest contributor"
    );
    let m = &f.message;
    assert!(m.starts_with("Always-on context for Copilot "), "{m}");
    assert_eq!(message_total(m), report.metrics.always_on_tokens, "{m}");
    let top = m.split("; top: ").nth(1).unwrap();
    let names: Vec<_> = top
        .split(", ")
        .map(|e| e.split(' ').next().unwrap())
        .collect();
    assert_eq!(names.len(), 6, "five contributors and `…`: {m}");
    assert_eq!(&names[..2], ["AGENTS.md", "CLAUDE.md"], "{m}");
    assert_eq!(names[5], "…", "{m}");
}

#[test]
fn budget_boundary() {
    for (tokens, expected) in [(8000, 0), (8001, 1)] {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "AGENTS.md", &prose("agents", tokens));
        let (report, _) = run(root);
        assert_eq!(report.metrics.always_on_tokens, u64::from(tokens));
        assert_eq!(
            always_on(&report).len(),
            expected,
            "{tokens}: {:#?}",
            report.findings
        );
    }
}

fn repo_fixtures() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/repos");
    let mut out: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir())
        .collect();
    out.sort();
    out
}

/// The report never contradicts itself: over the budget per `metrics.always_on_tokens` means
/// exactly one AI-EFF-002 finding, and a finding means a total over the budget. The rule's
/// total also counts files that always-loaded instructions `@import`, which the metric does
/// not, so it can only be larger.
#[test]
fn finding_agrees_with_the_always_on_metric() {
    let tmp: Vec<tempfile::TempDir> = (0..3).map(|_| tempfile::tempdir().unwrap()).collect();
    one_assistant_over(tmp[0].path());
    every_assistant_over(tmp[1].path());
    write(tmp[2].path(), "AGENTS.md", &prose("agents", 7990));
    let roots = repo_fixtures()
        .into_iter()
        .chain(tmp.iter().map(|t| t.path().to_path_buf()));

    let mut over = 0;
    for root in roots {
        let (report, config) = run(&root);
        let (metric, budget) = (report.metrics.always_on_tokens, config.budgets.always_on);
        let found = always_on(&report);
        let what = format!("{}: metric {metric}, budget {budget}", root.display());
        if metric > budget {
            over += 1;
            assert_eq!(found.len(), 1, "{what}: {:#?}", report.findings);
        }
        assert!(found.len() <= 1, "{what}");
        if let Some(f) = found.first() {
            let total = message_total(&f.message);
            assert!(total > budget && total >= metric, "{what}: {}", f.message);
        }
    }
    assert!(
        over >= 2,
        "the suite must include repositories over the budget"
    );
}
