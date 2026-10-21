//! PR-comment report in GitHub-flavored Markdown (spec 06-reports → Markdown).

use std::io::{self, Write};
use std::path::Path;

use pulse_core::{Category, Finding, Severity};
use pulse_engine::{GateStatus, Report};

use crate::rel_path;
use crate::terminal::{describe, pct, plural, tokens, upper};

/// At most this many findings are listed; the rest become a "+N more" line.
const MAX_FINDINGS: usize = 50;

/// How to render a [`Report`] as Markdown.
#[derive(Debug, Clone, Copy)]
pub struct MarkdownOptions<'a> {
    /// Scan root; finding paths are relative to it.
    pub root: &'a Path,
    /// Omit findings below this severity (counts and gate still include them).
    pub min_severity: Severity,
}

/// Writes `report` to `w` as Markdown.
pub fn render<W: Write + ?Sized>(
    w: &mut W,
    report: &Report,
    opts: &MarkdownOptions<'_>,
) -> io::Result<()> {
    let m = &report.metrics;
    let badge = match report.gate.status {
        GateStatus::Passed => "✅ PASSED",
        GateStatus::Failed => "❌ FAILED",
    };
    writeln!(w, "## Pulse Quality Gate: {badge}")?;
    writeln!(w)?;
    let assets: usize = m.assets_by_kind.values().sum();
    writeln!(
        w,
        "{} · {} tokens · {} always-on",
        plural(assets, "asset", "assets"),
        tokens(m.total_tokens),
        tokens(m.always_on_tokens)
    )?;

    if !report.gate.conditions.is_empty() {
        writeln!(w)?;
        writeln!(w, "| | Condition | Actual | Limit |")?;
        writeln!(w, "|:-:|---|---|---|")?;
        for cond in &report.gate.conditions {
            let (label, actual, limit) = describe(cond, report);
            let mark = if cond.passed { "✅" } else { "❌" };
            writeln!(
                w,
                "| {mark} | {} | {} | {} |",
                cell(&label),
                cell(&actual),
                cell(&limit)
            )?;
        }
    }

    writeln!(w)?;
    let r = &report.ratings;
    let count = |cat| m.issues_by_category.get(&cat).copied().unwrap_or(0);
    writeln!(w, "| Security | Reliability | Clarity | Efficiency |")?;
    writeln!(w, "|:-:|:-:|:-:|:-:|")?;
    writeln!(
        w,
        "| **{}** {} | **{}** {} | **{}** {} | **{}** context debt {} |",
        r.security,
        plural(
            count(Category::Security),
            "vulnerability",
            "vulnerabilities"
        ),
        r.reliability,
        plural(count(Category::Structure), "bug", "bugs"),
        r.clarity,
        plural(count(Category::Cognitive), "smell", "smells"),
        r.efficiency,
        pct(m.context_debt)
    )?;

    writeln!(w)?;
    let levels = Severity::ALL.iter().rev();
    let head: Vec<String> = levels.clone().map(|&s| upper(s)).collect();
    writeln!(w, "| {} |", head.join(" | "))?;
    writeln!(w, "|{}", ":-:|".repeat(head.len()))?;
    let counts: Vec<String> = levels
        .map(|s| {
            m.issues_by_severity
                .get(s)
                .copied()
                .unwrap_or(0)
                .to_string()
        })
        .collect();
    writeln!(w, "| {} |", counts.join(" | "))?;

    let shown: Vec<&Finding> = report
        .findings
        .iter()
        .filter(|f| f.severity >= opts.min_severity)
        .collect();
    findings(w, &shown, report.findings.len() - shown.len(), opts)
}

/// Findings grouped by file, one collapsible `<details>` each (they arrive sorted by path).
fn findings<W: Write + ?Sized>(
    w: &mut W,
    shown: &[&Finding],
    hidden: usize,
    opts: &MarkdownOptions<'_>,
) -> io::Result<()> {
    writeln!(w)?;
    if shown.is_empty() {
        if hidden == 0 {
            writeln!(w, "No issues found.")?;
        } else {
            writeln!(
                w,
                "No issues at {} or above ({hidden} hidden).",
                opts.min_severity
            )?;
        }
        return Ok(());
    }
    let listed = &shown[..shown.len().min(MAX_FINDINGS)];
    for group in listed.chunk_by(|a, b| a.location.path == b.location.path) {
        let path = rel_path(&group[0].location.path, opts.root);
        writeln!(
            w,
            "<details><summary><code>{}</code> ({})</summary>",
            html(&path),
            plural(group.len(), "issue", "issues")
        )?;
        writeln!(w)?;
        writeln!(w, "| Location | Severity | Rule | Message |")?;
        writeln!(w, "|---|---|---|---|")?;
        for f in group {
            writeln!(
                w,
                "| {}:{} | {} | `{}` | {} |",
                f.location.line,
                f.location.col,
                upper(f.severity),
                f.rule_id,
                cell(&f.message)
            )?;
        }
        writeln!(w)?;
        writeln!(w, "</details>")?;
    }
    let more = shown.len() - listed.len();
    if more > 0 {
        writeln!(w)?;
        writeln!(w, "+{more} more")?;
    }
    if hidden > 0 {
        writeln!(w)?;
        writeln!(
            w,
            "{} below {} hidden.",
            plural(hidden, "issue", "issues"),
            opts.min_severity
        )?;
    }
    Ok(())
}

/// `s` with HTML special characters escaped.
fn html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// `s` safe inside a table cell: HTML-escaped, `|` escaped, on one line.
fn cell(s: &str) -> String {
    html(s)
        .replace('|', "\\|")
        .replace("\r\n", " ")
        .replace(['\r', '\n'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_are_escaped_and_single_line() {
        assert_eq!(cell("a | b"), "a \\| b");
        assert_eq!(cell("<b>&</b>"), "&lt;b&gt;&amp;&lt;/b&gt;");
        assert_eq!(cell("one\r\ntwo\nthree"), "one two three");
    }
}
