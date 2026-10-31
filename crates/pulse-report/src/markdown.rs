//! PR-comment report in GitHub-flavored Markdown (spec 06-reports → Markdown).

use std::io::{self, Write};
use std::path::Path;

use pulse_core::{Category, Finding, Severity};
use pulse_engine::{GateStatus, Report};

use crate::context::{self, Level};
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
        "{} · {} tokens",
        plural(assets, "asset", "assets"),
        tokens(m.total_tokens)
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

    context_block(w, report)?;

    writeln!(w)?;
    // In new-code mode, ratings (and the counts beside them) cover new findings only.
    if let Some(new) = &report.new_metrics {
        writeln!(
            w,
            "New code: {} of {} issues are new (ratings and gate).",
            crate::issue_count(new),
            crate::issue_count(m)
        )?;
        writeln!(w)?;
    }
    let r = &report.ratings;
    let rm = report.new_metrics.as_ref().unwrap_or(m);
    let count = |cat| rm.issues_by_category.get(&cat).copied().unwrap_or(0);
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
        pct(rm.context_debt)
    )?;
    writeln!(w)?;
    writeln!(w, "Context debt: {}.", context::DEBT_MEANING)?;

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

/// The context-load block: headline with a status dot against the budget, a per-assistant
/// table when several assistants are present, what always loads and what loads on demand.
fn context_block<W: Write + ?Sized>(w: &mut W, report: &Report) -> io::Result<()> {
    let c = &report.context;
    writeln!(w)?;
    writeln!(
        w,
        "**Context load** (always-on context): {}",
        context::MEANING
    )?;
    writeln!(w)?;
    let (window_h, budget_h) = context::share_headers(c);
    writeln!(w, "| Assistant | Tokens | {window_h} | {budget_h} |")?;
    writeln!(w, "|---|--:|--:|--:|")?;
    for (name, t) in context::rows(c) {
        let dot = match context::level(t, c.budget) {
            Level::Ok => "🟢",
            Level::Near => "🟡",
            Level::Over => "🔴",
        };
        let (of_window, of_budget) = context::shares(c, t);
        writeln!(
            w,
            "| {dot} {name} | {} | {} | {} |",
            tokens(t),
            pct(of_window),
            pct(of_budget)
        )?;
    }
    writeln!(w)?;
    writeln!(
        w,
        "- Always loaded: {}",
        cell(&context::always_loaded(c, |p| format!("`{p}`")))
    )?;
    if c.on_demand_tokens > 0 {
        writeln!(w, "- On demand: {}", context::on_demand(c))?;
    }
    writeln!(
        w,
        "- Budget: {}; 🟡 above 75% of it, 🔴 above it.",
        context::budget(c)
    )?;
    if let (Some(scope), Some(left_out)) = (context::scope(report), context::not_loaded(report)) {
        writeln!(w, "- Scope: {scope}. {left_out}.")?;
    }
    Ok(())
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
