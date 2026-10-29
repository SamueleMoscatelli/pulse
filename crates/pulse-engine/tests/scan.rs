//! End-to-end scans through the engine pipeline.

#![allow(clippy::unwrap_used)] // helpers outside #[test] fns

use std::fs;
use std::path::{Path, PathBuf};

use pulse_core::{Config, Severity};
use pulse_engine::{Baseline, GateStatus, Report, ScanOptions, scan};

fn mixed() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/repos/mixed")
}

/// Everything in a report except timings, as comparable text.
fn render(r: &Report) -> String {
    let mut s = String::new();
    for f in &r.findings {
        s += &format!(
            "{} {}:{}:{}-{}:{} {} {} {:016x}\n",
            f.rule_id,
            f.location.path.display(),
            f.location.line,
            f.location.col,
            f.location.end_line,
            f.location.end_col,
            f.severity,
            f.message,
            f.fingerprint
        );
    }
    for a in &r.assets_meta {
        s += &format!("{a:?}\n");
    }
    s
}

#[test]
fn scan_mixed_is_deterministic() {
    let root = mixed();
    let config = Config::load(&root, None).unwrap();
    let first = render(&scan(&root, &config, &ScanOptions::default()).unwrap());
    assert!(first.contains("rel_path: \"CLAUDE.md\""), "{first}");
    for _ in 0..9 {
        let again = render(&scan(&root, &config, &ScanOptions::default()).unwrap());
        assert_eq!(first, again);
    }
}

fn write(dir: &Path, rel: &str, bytes: &[u8]) {
    let p = dir.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, bytes).unwrap();
}

#[test]
fn scan_reports_invalid_content_and_skipped_files() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "CLAUDE.md", b"# Rules\nUse \xff tabs.\n");
    write(root, ".mcp.json", br#"{ "mcpServers": [] }"#);
    write(root, "AGENTS.md", &[b'a'; 64]);
    write(
        root,
        ".claude/commands/ok.md",
        b"---\nname: a\ndescription: d\n---\n",
    );
    let mut config = Config::default();
    config.scan.max_file_bytes = 32;

    let report = scan(root, &config, &ScanOptions::default()).unwrap();
    let got: Vec<_> = report
        .findings
        .iter()
        .map(|f| {
            let rel = f.location.path.strip_prefix(root).unwrap();
            (
                rel.to_string_lossy().into_owned(),
                f.severity,
                f.fingerprint != 0,
            )
        })
        .collect();
    assert_eq!(
        got,
        [
            (".mcp.json".to_owned(), Severity::High, true),
            ("AGENTS.md".to_owned(), Severity::Info, true),
            ("CLAUDE.md".to_owned(), Severity::High, true),
        ]
    );
    assert_eq!(report.assets_meta.len(), 4);

    let only_claude = ScanOptions {
        paths: vec!["CLAUDE.md".into()],
        ..ScanOptions::default()
    };
    assert_eq!(scan(root, &config, &only_claude).unwrap().findings.len(), 1);
    let skip = ScanOptions {
        skip_rules: vec!["AI-STR".into()],
        ..ScanOptions::default()
    };
    assert!(scan(root, &config, &skip).unwrap().findings.is_empty());

    let low = Config::parse(
        "[scan]\nmax_file_bytes = 32\n[rules]\nAI-STR-002 = \"low\"\n",
        Path::new("pulse.toml"),
    )
    .unwrap();
    let report = scan(root, &low, &ScanOptions::default()).unwrap();
    assert!(report.findings.iter().all(|f| f.severity == Severity::Low));
}

#[test]
fn scan_rejects_unknown_rule_in_config() {
    let config = Config::parse("[rules]\nAI-STR-02 = \"off\"\n", Path::new("pulse.toml")).unwrap();
    let err = scan(&mixed(), &config, &ScanOptions::default()).unwrap_err();
    assert!(
        err.to_string().contains("did you mean `AI-STR-002`"),
        "{err}"
    );
}

#[test]
fn scan_applies_inline_and_config_suppressions() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let bad_yaml = "---\na: [\n---\n";
    write(
        root,
        "CLAUDE.md",
        format!("{bad_yaml}<!-- pulse-ignore-file AI-STR-002 -- legacy -->\n# T\n").as_bytes(),
    );
    write(
        root,
        "docs/AGENTS.md",
        format!("{bad_yaml}# T\n").as_bytes(),
    );
    write(root, "AGENTS.md", format!("{bad_yaml}# T\n").as_bytes());
    let config = Config::parse(
        "[[ignore]]\npath = \"docs/**\"\nrules = [\"AI-STR-002\"]\n",
        Path::new("pulse.toml"),
    )
    .unwrap();
    let r = scan(root, &config, &ScanOptions::default()).unwrap();
    let got: Vec<_> = r
        .findings
        .iter()
        .map(|f| f.location.path.strip_prefix(root).unwrap().to_owned())
        .collect();
    assert_eq!(got, [PathBuf::from("AGENTS.md")], "{}", render(&r));
    assert_eq!(r.suppressed, 2);
}

#[test]
fn scan_keeps_sec003_over_sec004_on_same_span() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    // A provider-shaped key, built here so no secret-like literal is committed.
    let key = ["sk-", "proj-", &"Ab3xQ9".repeat(7)].concat();
    let json = format!(
        "{{\"mcpServers\": {{\"ai\": {{\"command\": \"npx\", \"args\": [\"openai-mcp@1.4.2\"],\n  \"env\": {{\"OPENAI_API_KEY\": \"{key}\", \"DB_PASSWORD\": \"hunter2-prod\"}}}}}}}}\n"
    );
    write(root, ".mcp.json", json.as_bytes());
    let r = scan(root, &Config::default(), &ScanOptions::default()).unwrap();
    let got: Vec<_> = r
        .findings
        .iter()
        .filter(|f| f.rule_id.starts_with("AI-SEC-00"))
        .map(|f| (f.rule_id, f.location.line, f.location.col))
        .collect();
    let key_col = u32::try_from(json.lines().nth(1).unwrap().find(&key).unwrap() + 1).unwrap();
    let pw_col = u32::try_from(json.lines().nth(1).unwrap().find("hunter2").unwrap() + 1).unwrap();
    assert_eq!(
        got,
        [("AI-SEC-003", 2, key_col), ("AI-SEC-004", 2, pw_col)],
        "{}",
        render(&r)
    );
}

#[test]
fn scan_scores_and_gates_in_scope_assets() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "CLAUDE.md",
        "Use tabs for indentation.\n".repeat(20).as_bytes(),
    );
    write(
        root,
        "sub/AGENTS.md",
        "Write tests first.\n".repeat(50).as_bytes(),
    );
    let config = Config::parse("[budgets]\ninstructions = 10\n", Path::new("pulse.toml")).unwrap();
    let opts = ScanOptions {
        paths: vec!["CLAUDE.md".into()],
        ..ScanOptions::default()
    };
    let r = scan(root, &config, &opts).unwrap();
    let claude = r
        .assets_meta
        .iter()
        .find(|a| a.rel_path == "CLAUDE.md")
        .unwrap();
    assert_eq!(r.assets_meta.len(), 2);
    assert_eq!(r.metrics.total_tokens, u64::from(claude.tokens));
    assert!(r.metrics.context_debt > 0.10, "{:?}", r.metrics);
    assert_eq!(r.gate.status, GateStatus::Failed, "{:?}", r.gate);
    let debt = r
        .gate
        .conditions
        .iter()
        .find(|c| c.name == "max_context_debt")
        .unwrap();
    assert!(!debt.passed);
}

/// A CLAUDE.md with `lead` lines before one zero-width-space finding (AI-SEC-001, critical).
fn claude_md(lead: &str, extra: &str) -> String {
    format!(
        "# Project\n{lead}\nAlways run{}the tests.\n{extra}",
        '\u{200B}'
    )
}

fn new_only(baseline: &Baseline) -> ScanOptions {
    ScanOptions {
        baseline: Some(baseline.clone()),
        new_only: true,
        ..ScanOptions::default()
    }
}

#[test]
fn baseline_survives_shifted_lines() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let config = Config::default();
    write(root, "CLAUDE.md", claude_md("", "").as_bytes());
    let before = scan(root, &config, &ScanOptions::default()).unwrap();
    assert!(before.findings.iter().any(|f| f.rule_id == "AI-SEC-001"));
    let baseline = Baseline::from_findings(&before.findings);

    write(
        root,
        "CLAUDE.md",
        claude_md("\nBuild with cargo.\n\n", "").as_bytes(),
    );
    let after = scan(root, &config, &new_only(&baseline)).unwrap();
    assert!(after.findings.is_empty(), "{}", render(&after));
    assert_eq!(after.gate.status, GateStatus::Passed);
    // Overall metrics still count the baselined finding; new-code metrics do not.
    assert_eq!(
        after.metrics.issues_by_severity.get(&Severity::Critical),
        Some(&1)
    );
    let new = after.new_metrics.unwrap();
    assert_eq!(new.issues_by_severity.get(&Severity::Critical), None);
}

#[test]
fn new_only_keeps_findings_missing_from_the_baseline() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let config = Config::default();
    write(root, "CLAUDE.md", claude_md("", "").as_bytes());
    let baseline = Baseline::from_findings(
        &scan(root, &config, &ScanOptions::default())
            .unwrap()
            .findings,
    );

    // A second, identical line is a new occurrence of the same (rule, path, text) triple.
    let extra = format!("Always run{}the tests.\n", '\u{200B}');
    write(root, "CLAUDE.md", claude_md("", &extra).as_bytes());
    let all = ScanOptions {
        baseline: Some(baseline.clone()),
        ..ScanOptions::default()
    };
    let full = scan(root, &config, &all).unwrap();
    let sec = |r: &Report| {
        r.findings
            .iter()
            .filter(|f| f.rule_id == "AI-SEC-001")
            .map(|f| (f.location.line, r.is_new(f)))
            .collect::<Vec<_>>()
    };
    assert_eq!(sec(&full), [(3, false), (4, true)]);
    assert!(full.new_metrics.is_none());

    let only = scan(root, &config, &new_only(&baseline)).unwrap();
    assert_eq!(sec(&only), [(4, true)]);
    assert!(only.findings.iter().all(|f| only.is_new(f)));
    assert_eq!(only.gate.status, GateStatus::Failed);
}

#[test]
fn gate_new_code_only_ignores_baselined_findings() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "CLAUDE.md", claude_md("", "").as_bytes());
    let plain = Config::default();
    let first = scan(root, &plain, &ScanOptions::default()).unwrap();
    assert_eq!(first.gate.status, GateStatus::Failed);
    let opts = ScanOptions {
        baseline: Some(Baseline::from_findings(&first.findings)),
        ..ScanOptions::default()
    };
    // Without new-code mode the baseline only marks findings; the gate still fails.
    assert_eq!(
        scan(root, &plain, &opts).unwrap().gate.status,
        GateStatus::Failed
    );

    let new_code =
        Config::parse("[gate]\nnew_code_only = true\n", Path::new("pulse.toml")).unwrap();
    let report = scan(root, &new_code, &opts).unwrap();
    assert_eq!(report.gate.status, GateStatus::Passed);
    // Every finding is still listed, marked as not new.
    assert_eq!(report.findings.len(), first.findings.len());
    assert!(report.findings.iter().all(|f| !report.is_new(f)));
}

#[test]
fn changed_set_limits_findings_to_those_files() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "CLAUDE.md", claude_md("", "").as_bytes());
    write(root, "AGENTS.md", claude_md("", "").as_bytes());
    let config = Config::default();
    let opts = ScanOptions {
        changed: Some(["AGENTS.md".to_owned()].into()),
        ..ScanOptions::default()
    };
    let report = scan(root, &config, &opts).unwrap();
    assert!(!report.findings.is_empty());
    assert!(
        report
            .findings
            .iter()
            .all(|f| f.location.path.ends_with("AGENTS.md")),
        "{}",
        render(&report)
    );
    let none = ScanOptions {
        changed: Some(Default::default()),
        ..ScanOptions::default()
    };
    assert!(scan(root, &config, &none).unwrap().findings.is_empty());
}

#[test]
fn custom_rules_run_with_config_overrides() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "CLAUDE.md",
        b"# Rules\n\nDeploy to build.internal.corp only.\nNever say Ship It without tests.\n",
    );
    write(
        root,
        ".claude/skills/s/SKILL.md",
        b"---\nname: s\ndescription: Use when deploying.\n---\n\nAsk build.internal.corp.\n",
    );
    let src = r#"
[rules]
X-ORG-002 = "low"

[[custom_rules]]
id = "X-ORG-001"
name = "No internal hostnames"
severity = "high"
kinds = ["instructions"]
pattern = '\binternal\.corp\b'
message = "Internal hostname leaked into AI context"
remediation = "Use the public alias"

[[custom_rules]]
id = "X-ORG-002"
name = "No slogans"
severity = "medium"
phrases = ["ship it"]
message = "Slogan in instructions"
"#;
    let config = Config::parse(src, Path::new("pulse.toml")).unwrap();
    let report = scan(root, &config, &ScanOptions::default()).unwrap();
    let custom: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule_id.starts_with("X-"))
        .map(|f| {
            let rel = f.location.path.strip_prefix(root).unwrap();
            (
                f.rule_id,
                rel.to_string_lossy().replace('\\', "/"),
                f.location.line,
                f.severity,
            )
        })
        .collect();
    // X-ORG-001 is limited to instructions, so the skill's hostname is not reported.
    assert_eq!(
        custom,
        [
            ("X-ORG-001", "CLAUDE.md".to_owned(), 3, Severity::High),
            ("X-ORG-002", "CLAUDE.md".to_owned(), 4, Severity::Low),
        ]
    );
    let f = report
        .findings
        .iter()
        .find(|f| f.rule_id == "X-ORG-001")
        .unwrap();
    assert_eq!(f.remediation, Some("Use the public alias"));

    let off = format!("{src}\n").replace("X-ORG-002 = \"low\"", "X-ORG-002 = \"off\"");
    let config = Config::parse(&off, Path::new("pulse.toml")).unwrap();
    let report = scan(root, &config, &ScanOptions::default()).unwrap();
    assert!(report.findings.iter().all(|f| f.rule_id != "X-ORG-002"));
    let only = ScanOptions {
        only_rules: vec!["X-".into()],
        ..ScanOptions::default()
    };
    let report = scan(root, &config, &only).unwrap();
    assert!(report.findings.iter().all(|f| f.rule_id == "X-ORG-001"));
    assert!(!report.findings.is_empty());
}

#[test]
fn custom_rule_bad_regex_is_a_config_error() {
    let dir = tempfile::tempdir().unwrap();
    let src = "[[custom_rules]]\nid = \"X-ORG-001\"\nname = \"n\"\nseverity = \"low\"\npattern = '(oops'\nmessage = \"m\"\n";
    let config = Config::parse(src, Path::new("pulse.toml")).unwrap();
    let err = scan(dir.path(), &config, &ScanOptions::default())
        .err()
        .unwrap();
    assert!(err.to_string().contains("custom rule `X-ORG-001`"), "{err}");
    let opts = "[rules.X-ORG-001]\nfoo = 1\n";
    let config = Config::parse(
        &format!("{opts}{src}").replace("'(oops'", "'x'"),
        Path::new("p"),
    )
    .unwrap();
    let err = scan(dir.path(), &config, &ScanOptions::default())
        .err()
        .unwrap();
    assert!(
        err.to_string().contains("custom rules take no options"),
        "{err}"
    );
}

#[test]
fn scan_library_assets_add_no_always_on_tokens() {
    use pulse_core::LoadMode;
    let root = mixed().with_file_name("library");
    let config = Config::load(&root, None).unwrap();
    let report = scan(&root, &config, &ScanOptions::default()).unwrap();
    let meta = |rel: &str| {
        report
            .assets_meta
            .iter()
            .find(|a| a.rel_path == rel)
            .unwrap()
    };
    // `applyTo: '**'` would promote it to Always under `.github/instructions/`.
    let lib = meta("instructions/everywhere.instructions.md");
    assert_eq!((lib.load, lib.always_tokens), (LoadMode::OnDemand, 0));
    let claude = meta("CLAUDE.md");
    assert_eq!(claude.load, LoadMode::Always);
    assert_eq!(
        report.metrics.always_on_tokens,
        u64::from(claude.always_tokens)
    );
}
