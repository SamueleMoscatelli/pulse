//! End-to-end scans through the engine pipeline.

#![allow(clippy::unwrap_used)] // helpers outside #[test] fns

use std::fs;
use std::path::{Path, PathBuf};

use pulse_core::{Config, Severity};
use pulse_engine::{GateStatus, Report, ScanOptions, scan};

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
        ".claude/agents/ok.md",
        b"---\ndescription: fine\n---\nBody\n",
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
    let config = Config::parse("[rules]\nAI-STR-020 = \"off\"\n", Path::new("pulse.toml")).unwrap();
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
