//! The default, human-oriented report (spec 06-reports → Terminal).

use std::fmt;
use std::io::{self, Write};
use std::path::Path;

use anstyle::{Ansi256Color, AnsiColor, Color, Style};
use pulse_core::{Finding, Rating, Severity};
use pulse_engine::{Condition, GateStatus, GateValue, Report};

use crate::rel_path;

/// How to render a [`Report`] for the terminal.
#[derive(Debug, Clone, Copy)]
pub struct TerminalOptions<'a> {
    /// Scan root; finding paths are shown relative to it.
    pub root: &'a Path,
    /// Emit ANSI styles.
    pub color: bool,
    /// Findings only, one grep-friendly line each.
    pub quiet: bool,
    /// Show the remediation of every finding, not only Critical and High.
    pub verbose: bool,
    /// Hide findings below this severity (display only).
    pub min_severity: Severity,
    /// `context.window`, for the context-load line.
    pub window: u64,
}

const BOLD: Style = Style::new().bold();
const DIM: Style = Style::new().dimmed();
const GREEN: Style = fg(Color::Ansi(AnsiColor::Green));
const RED: Style = fg(Color::Ansi(AnsiColor::Red));

const fn fg(c: Color) -> Style {
    Style::new().fg_color(Some(c))
}

/// Text wrapped in a style when color is on.
struct Paint<T>(Style, T, bool);

impl<T: fmt::Display> fmt::Display for Paint<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.2 {
            write!(f, "{}{}{}", self.0.render(), self.1, self.0.render_reset())
        } else {
            self.1.fmt(f)
        }
    }
}

/// Paints text when color is on.
#[derive(Clone, Copy)]
struct Painter(bool);

impl Painter {
    fn paint<T: fmt::Display>(self, style: Style, text: T) -> Paint<T> {
        Paint(style, text, self.0)
    }
}

/// Writes `report` to `w`.
pub fn render<W: Write + ?Sized>(
    w: &mut W,
    report: &Report,
    opts: &TerminalOptions<'_>,
) -> io::Result<()> {
    let shown: Vec<&Finding> = report
        .findings
        .iter()
        .filter(|f| f.severity >= opts.min_severity)
        .collect();
    if opts.quiet {
        return quiet(w, &shown, opts);
    }
    let c = opts.color;
    let p = Painter(c);
    let m = &report.metrics;

    let assets: usize = m.assets_by_kind.values().sum();
    writeln!(
        w,
        " {} {} · {} · {} tokens · {} ms",
        p.paint(BOLD, "Pulse"),
        env!("CARGO_PKG_VERSION"),
        plural(assets, "asset", "assets"),
        tokens(m.total_tokens),
        report.timings.total.as_millis()
    )?;

    writeln!(w)?;
    let (status, style) = match report.gate.status {
        GateStatus::Passed => (
            "PASSED",
            Style::new().bold().bg_color(Some(AnsiColor::Green.into())),
        ),
        GateStatus::Failed => (
            "FAILED",
            Style::new().bold().bg_color(Some(AnsiColor::Red.into())),
        ),
    };
    let style = style.fg_color(Some(AnsiColor::Black.into()));
    if c {
        // The padding lives inside the badge; no trailing spaces without color.
        writeln!(
            w,
            " {}  {}",
            p.paint(BOLD, "QUALITY GATE"),
            p.paint(style, format!(" {status} "))
        )?;
    } else {
        writeln!(w, " QUALITY GATE   {status}")?;
    }
    for cond in &report.gate.conditions {
        let (label, actual, limit) = describe(cond, report);
        let mark = if cond.passed {
            p.paint(GREEN, "✔")
        } else {
            p.paint(RED, "✖")
        };
        writeln!(w, "   {mark} {label:<23}{actual:<11}({limit})")?;
    }

    writeln!(w)?;
    // In new-code mode, ratings (and the counts beside them) cover new findings only.
    if let Some(new) = &report.new_metrics {
        writeln!(
            w,
            "   {:<14}{} of {} issues are new (ratings and gate)",
            "New code",
            crate::issue_count(new),
            crate::issue_count(m)
        )?;
    }
    let r = &report.ratings;
    let rm = report.new_metrics.as_ref().unwrap_or(m);
    let count = |cat| rm.issues_by_category.get(&cat).copied().unwrap_or(0);
    use pulse_core::Category::{Cognitive, Security, Structure};
    let dims = [
        (
            "Security",
            r.security,
            plural(count(Security), "vulnerability", "vulnerabilities"),
        ),
        (
            "Reliability",
            r.reliability,
            plural(count(Structure), "bug", "bugs"),
        ),
        (
            "Clarity",
            r.clarity,
            plural(count(Cognitive), "smell", "smells"),
        ),
        (
            "Efficiency",
            r.efficiency,
            format!("context debt {}", pct(rm.context_debt)),
        ),
    ];
    for (name, rating, detail) in dims {
        writeln!(
            w,
            "   {name:<14}{}   {detail}",
            p.paint(rating_style(rating), rating)
        )?;
    }
    writeln!(
        w,
        "   {:<18}{} always-on tokens · {} of {} window",
        "Context load",
        tokens(m.always_on_tokens),
        pct(m.context_load),
        tokens(opts.window)
    )?;

    writeln!(w)?;
    let counts: Vec<String> = Severity::ALL
        .iter()
        .rev()
        .map(|&s| {
            let n = m.issues_by_severity.get(&s).copied().unwrap_or(0);
            format!("{} {n}", p.paint(severity_style(s), upper(s)))
        })
        .collect();
    writeln!(w, "   {}", counts.join("   "))?;

    findings(w, &shown, report.findings.len() - shown.len(), opts)
}

/// The findings, grouped by file (they arrive sorted by path).
fn findings<W: Write + ?Sized>(
    w: &mut W,
    shown: &[&Finding],
    hidden: usize,
    opts: &TerminalOptions<'_>,
) -> io::Result<()> {
    let c = opts.color;
    let p = Painter(c);
    if shown.is_empty() {
        writeln!(w)?;
        if hidden == 0 {
            writeln!(w, " No issues found.")?;
        } else {
            writeln!(
                w,
                " No issues at {} or above ({hidden} hidden).",
                opts.min_severity
            )?;
        }
        return Ok(());
    }
    let loc = |f: &Finding| format!("{}:{}", f.location.line, f.location.col);
    let loc_w = shown.iter().map(|f| loc(f).len()).max().unwrap_or(0).max(6) + 2;
    let rule_w = shown.iter().map(|f| f.rule_id.len()).max().unwrap_or(0) + 2;
    let indent = " ".repeat(3 + loc_w + 10);

    let mut current: Option<&Path> = None;
    for f in shown {
        if current != Some(&*f.location.path) {
            current = Some(&f.location.path);
            writeln!(w)?;
            writeln!(
                w,
                " {}",
                p.paint(BOLD, rel_path(&f.location.path, opts.root))
            )?;
        }
        writeln!(
            w,
            "   {:<loc_w$}{}{}{}",
            loc(f),
            p.paint(
                severity_style(f.severity),
                format!("{:<10}", upper(f.severity))
            ),
            p.paint(DIM, format!("{:<rule_w$}", f.rule_id)),
            f.message
        )?;
        let wants_fix = opts.verbose || f.severity >= Severity::High;
        if let Some(fix) = f.remediation.filter(|_| wants_fix) {
            writeln!(w, "{indent}{} {fix}", p.paint(DIM, "↳"))?;
        }
    }
    if hidden > 0 {
        writeln!(w)?;
        writeln!(
            w,
            " {} below {} hidden.",
            plural(hidden, "issue", "issues"),
            opts.min_severity
        )?;
    }
    Ok(())
}

/// `-q`: `path:line:col: SEVERITY RULE message`.
fn quiet<W: Write + ?Sized>(
    w: &mut W,
    shown: &[&Finding],
    opts: &TerminalOptions<'_>,
) -> io::Result<()> {
    for f in shown {
        writeln!(
            w,
            "{}:{}:{}: {} {} {}",
            rel_path(&f.location.path, opts.root),
            f.location.line,
            f.location.col,
            Paint(severity_style(f.severity), upper(f.severity), opts.color),
            f.rule_id,
            f.message
        )?;
    }
    Ok(())
}

/// Label, actual value and limit of a gate condition.
pub(crate) fn describe(cond: &Condition, report: &Report) -> (String, String, String) {
    let ratio = |v: &GateValue| match *v {
        GateValue::Ratio(r) => pct(r),
        ref other => other.to_string(),
    };
    let tok = |v: &GateValue| match *v {
        GateValue::Tokens(t) => tokens(t),
        ref other => other.to_string(),
    };
    match cond.name {
        "fail_on_severity" => {
            let n = match cond.threshold {
                GateValue::Severity(min) => {
                    report.findings.iter().filter(|f| f.severity >= min).count()
                }
                _ => 0,
            };
            (
                format!("severity ≥ {}", cond.threshold),
                plural(n, "issue", "issues"),
                "allowed 0".to_owned(),
            )
        }
        "max_context_debt" => (
            "context debt".to_owned(),
            ratio(&cond.actual),
            format!("max {}", ratio(&cond.threshold)),
        ),
        "max_always_on_tokens" => (
            "always-on tokens".to_owned(),
            tok(&cond.actual),
            format!("max {}", tok(&cond.threshold)),
        ),
        name => (
            format!(
                "{} rating",
                name.strip_prefix("min_ratings.").unwrap_or(name)
            ),
            cond.actual.to_string(),
            format!("min {}", cond.threshold),
        ),
    }
}

pub(crate) fn upper(s: Severity) -> String {
    s.as_str().to_ascii_uppercase()
}

pub(crate) fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// `842`, `61.2k`, `200k`, `1.5M`.
pub(crate) fn tokens(n: u64) -> String {
    let (v, unit) = match n {
        0..1_000 => return n.to_string(),
        1_000..1_000_000 => (n as f64 / 1e3, "k"),
        _ => (n as f64 / 1e6, "M"),
    };
    format!("{}{unit}", trim_zero(format!("{v:.1}")))
}

/// A fraction as a percentage with one decimal: `6.1%`, `10%`.
pub(crate) fn pct(r: f64) -> String {
    format!("{}%", trim_zero(format!("{:.1}", r * 100.0)))
}

fn trim_zero(s: String) -> String {
    match s.strip_suffix(".0") {
        Some(t) => t.to_owned(),
        None => s,
    }
}

fn severity_style(s: Severity) -> Style {
    match s {
        Severity::Critical => RED.bold(),
        Severity::High => RED,
        Severity::Medium => fg(Color::Ansi(AnsiColor::Yellow)),
        Severity::Low => fg(Color::Ansi(AnsiColor::Cyan)),
        Severity::Info => DIM,
    }
}

fn rating_style(r: Rating) -> Style {
    let color = match r {
        Rating::A => Color::Ansi(AnsiColor::Green),
        Rating::B => Color::Ansi256(Ansi256Color(148)),
        Rating::C => Color::Ansi(AnsiColor::Yellow),
        Rating::D => Color::Ansi256(Ansi256Color(208)),
        Rating::E => Color::Ansi(AnsiColor::Red),
    };
    fg(color).bold()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_token_counts() {
        assert_eq!(tokens(842), "842");
        assert_eq!(tokens(61_234), "61.2k");
        assert_eq!(tokens(200_000), "200k");
        assert_eq!(tokens(1_500_000), "1.5M");
    }

    #[test]
    fn formats_percentages() {
        assert_eq!(pct(0.061), "6.1%");
        assert_eq!(pct(0.10), "10%");
        assert_eq!(pct(0.0), "0%");
    }

    #[test]
    fn paths_are_root_relative_with_slashes() {
        let root = Path::new("/r");
        assert_eq!(rel_path(Path::new("/r/a/CLAUDE.md"), root), "a/CLAUDE.md");
        assert_eq!(rel_path(Path::new("x.md"), root), "x.md");
    }

    #[test]
    fn paint_is_plain_without_color() {
        assert_eq!(Paint(RED, "x", false).to_string(), "x");
        assert_ne!(Paint(RED, "x", true).to_string(), "x");
    }
}
