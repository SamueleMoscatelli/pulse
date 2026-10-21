//! GitHub Actions workflow commands (spec 06-reports → GitHub).

use std::io::{self, Write};
use std::path::Path;

use pulse_core::Severity;
use pulse_engine::{GateStatus, Report};

use crate::rel_path;
use crate::terminal::{describe, upper};

/// How to render a [`Report`] as GitHub Actions annotations.
#[derive(Debug, Clone, Copy)]
pub struct GithubOptions<'a> {
    /// Scan root; annotation paths are relative to it.
    pub root: &'a Path,
    /// Skip findings below this severity (the summary still counts them).
    pub min_severity: Severity,
}

/// Writes one workflow command per finding, then a plain-text gate summary.
pub fn render<W: Write + ?Sized>(
    w: &mut W,
    report: &Report,
    opts: &GithubOptions<'_>,
) -> io::Result<()> {
    for f in report
        .findings
        .iter()
        .filter(|f| f.severity >= opts.min_severity)
    {
        let level = match f.severity {
            Severity::Critical | Severity::High => "error",
            Severity::Medium => "warning",
            Severity::Low | Severity::Info => "notice",
        };
        let l = &f.location;
        writeln!(
            w,
            "::{level} file={},line={},col={},title={}::{}",
            property(&rel_path(&l.path, opts.root)),
            l.line,
            l.col,
            property(f.rule_id),
            data(&f.message)
        )?;
    }

    let status = match report.gate.status {
        GateStatus::Passed => "PASSED",
        GateStatus::Failed => "FAILED",
    };
    writeln!(w, "Pulse Quality Gate: {status}")?;
    for cond in &report.gate.conditions {
        let (label, actual, limit) = describe(cond, report);
        let mark = if cond.passed { "✔" } else { "✖" };
        writeln!(w, "  {mark} {label}: {actual} ({limit})")?;
    }
    let m = &report.metrics;
    let counts: Vec<String> = Severity::ALL
        .iter()
        .rev()
        .map(|&s| {
            let n = m.issues_by_severity.get(&s).copied().unwrap_or(0);
            format!("{} {n}", upper(s))
        })
        .collect();
    writeln!(w, "  {}", counts.join("   "))
}

/// Escapes a workflow command message (`%`, CR, LF).
fn data(s: &str) -> String {
    s.replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

/// Escapes a workflow command property value: as [`data`], plus `:` and `,`.
fn property(s: &str) -> String {
    data(s).replace(':', "%3A").replace(',', "%2C")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_follow_workflow_command_rules() {
        assert_eq!(data("100% a:b,c\r\nd"), "100%25 a:b,c%0D%0Ad");
        assert_eq!(property("a:b,c%\n"), "a%3Ab%2Cc%25%0A");
        assert_eq!(data("%0A"), "%250A");
    }
}
