//! SARIF 2.1.0 report for GitHub Code Scanning (spec 06-reports → SARIF).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::{self, Write};
use std::path::Path;

use pulse_core::{Category, Finding, RuleMeta, Severity};
use pulse_engine::Report;
use serde_json::{Value, json};

use crate::rel_path;

/// How to render a [`Report`] as SARIF.
#[derive(Debug, Clone, Copy)]
pub struct SarifOptions<'a> {
    /// Scan root; artifact URIs are relative to it (`%SRCROOT%`).
    pub root: &'a Path,
    /// Omit findings below this severity.
    pub min_severity: Severity,
}

/// Writes `report` to `w` as a pretty-printed SARIF 2.1.0 log, with a trailing newline.
pub fn render<W: Write + ?Sized>(
    w: &mut W,
    report: &Report,
    opts: &SarifOptions<'_>,
) -> io::Result<()> {
    let shown: Vec<&Finding> = report
        .findings
        .iter()
        .filter(|f| f.severity >= opts.min_severity)
        .collect();
    // Only the rules that have results, in ID order; `ruleIndex` points into this list.
    let index: BTreeMap<&str, usize> = {
        let mut ids: Vec<&str> = shown.iter().map(|f| f.rule_id).collect();
        ids.sort_unstable();
        ids.dedup();
        ids.into_iter().enumerate().map(|(i, id)| (id, i)).collect()
    };
    let rules: Vec<Value> = index.keys().map(|id| rule(id)).collect();
    let results: Vec<Value> = shown
        .iter()
        .map(|f| result(f, index.get(f.rule_id).copied().unwrap_or(0), opts.root))
        .collect();
    let log = json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": { "driver": {
                "name": "pulse",
                "version": env!("CARGO_PKG_VERSION"),
                "informationUri": env!("CARGO_PKG_REPOSITORY"),
                "rules": rules,
            }},
            "columnKind": "unicodeCodePoints",
            "results": results,
        }],
    });
    serde_json::to_writer_pretty(&mut *w, &log)?;
    writeln!(w)
}

/// `reportingDescriptor` of a rule; custom rules (not in the registry) get only their ID.
fn rule(id: &str) -> Value {
    let Some(m) = pulse_rules::find(id) else {
        return json!({ "id": id });
    };
    let mut properties = json!({ "tags": [m.category.as_str()] });
    if m.category == Category::Security {
        properties["security-severity"] = json!(security_severity(m.default_severity));
    }
    json!({
        "id": m.id,
        "name": m.name,
        "shortDescription": { "text": m.summary },
        "help": help(m),
        "defaultConfiguration": { "level": level(m.default_severity) },
        "properties": properties,
    })
}

fn help(m: &RuleMeta) -> Value {
    match pulse_rules::docs(m.id) {
        Some(md) => json!({ "text": m.remediation, "markdown": md }),
        None => json!({ "text": m.remediation }),
    }
}

fn result(f: &Finding, rule_index: usize, root: &Path) -> Value {
    let l = &f.location;
    // SARIF regions must not end before they start.
    let (line, col) = (l.line.max(1), l.col.max(1));
    let end_line = l.end_line.max(line);
    let end_col = if end_line == line {
        l.end_col.max(col)
    } else {
        l.end_col.max(1)
    };
    json!({
        "ruleId": f.rule_id,
        "ruleIndex": rule_index,
        "level": level(f.severity),
        "message": { "text": f.message },
        "locations": [{ "physicalLocation": {
            "artifactLocation": { "uri": uri(&rel_path(&l.path, root)), "uriBaseId": "%SRCROOT%" },
            "region": {
                "startLine": line,
                "startColumn": col,
                "endLine": end_line,
                "endColumn": end_col,
            },
        }}],
        "partialFingerprints": { "pulse/v1": crate::fingerprint(f) },
    })
}

/// Critical/High → `error`, Medium → `warning`, Low/Info → `note`.
fn level(s: Severity) -> &'static str {
    match s {
        Severity::Critical | Severity::High => "error",
        Severity::Medium => "warning",
        Severity::Low | Severity::Info => "note",
    }
}

/// GitHub's `security-severity` score (a numeric string).
fn security_severity(s: Severity) -> &'static str {
    match s {
        Severity::Critical => "9.5",
        Severity::High => "8.0",
        Severity::Medium => "5.5",
        Severity::Low | Severity::Info => "3.0",
    }
}

/// A relative path as a URI reference: bytes outside the unreserved set and `/` are
/// percent-encoded.
fn uri(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for b in path.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~/".contains(&b) {
            out.push(char::from(b));
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uri_percent_encodes_reserved_and_non_ascii() {
        assert_eq!(uri(".claude/a b.md"), ".claude/a%20b.md");
        assert_eq!(uri("dir/é#1.md"), "dir/%C3%A9%231.md");
    }

    #[test]
    fn rule_scores_only_security_rules() {
        let sec = rule("AI-SEC-001");
        assert_eq!(sec["properties"]["security-severity"], "9.5");
        assert_eq!(sec["defaultConfiguration"]["level"], "error");
        assert!(sec["help"]["markdown"].as_str().is_some());
        assert!(
            rule("AI-COG-001")["properties"]
                .get("security-severity")
                .is_none()
        );
        assert_eq!(rule("CUSTOM-001"), json!({ "id": "CUSTOM-001" }));
    }

    #[test]
    fn level_maps_severities() {
        assert_eq!(level(Severity::High), "error");
        assert_eq!(level(Severity::Medium), "warning");
        assert_eq!(level(Severity::Info), "note");
    }
}
