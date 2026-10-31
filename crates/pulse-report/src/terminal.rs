//! The default, human-oriented report (spec 06-reports → Terminal).

use std::fmt;
use std::io::{self, Write};
use std::path::Path;

use anstyle::{Ansi256Color, AnsiColor, Color, Style};
use pulse_core::{Finding, Rating, Severity};
use pulse_engine::{Condition, GateStatus, GateValue, Report};

use crate::context::{self, Level};
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
    /// Show the remediation of every finding, not only Medium and higher.
    pub verbose: bool,
    /// Hide findings below this severity (display only).
    pub min_severity: Severity,
    /// Terminal width: long messages and fixes wrap under their own column. `None` keeps
    /// one line per finding (pipes, files).
    pub width: Option<usize>,
}

const BOLD: Style = Style::new().bold();
const DIM: Style = Style::new().dimmed();
const GREEN: Style = fg(Color::Ansi(AnsiColor::Green));
const RED: Style = fg(Color::Ansi(AnsiColor::Red));
const YELLOW: Style = fg(Color::Ansi(AnsiColor::Yellow));

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

    scope_block(w, report, p, opts)?;
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
    context_block(w, report, p, opts.width)?;

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
            format!(
                "context debt {} · {}",
                pct(rm.context_debt),
                context::DEBT_MEANING
            ),
        ),
    ];
    for (name, rating, detail) in dims {
        writeln!(
            w,
            "   {name:<14}{}   {}",
            p.paint(rating_style(rating), rating),
            wrap(&detail, 21, opts.width)
        )?;
    }

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
    let fixes_hidden = shown
        .iter()
        .any(|f| f.remediation.is_some() && !shows_fix(f, opts));
    if fixes_hidden {
        writeln!(
            w,
            "   {}",
            p.paint(
                DIM,
                "Fixes shown for medium and higher · run pulse scan -v to see all"
            )
        )?;
    }

    findings(w, &shown, report.findings.len() - shown.len(), opts)?;
    if fixes_hidden {
        writeln!(w)?;
        let note = format!(
            "Fixes shown for medium and higher findings. Run with -v to see fixes for all {}, \
             or `pulse explain <RULE-ID>` for a rule's full guidance.",
            plural(shown.len(), "finding", "findings")
        );
        writeln!(w, " {} {}", p.paint(DIM, "↳"), wrap(&note, 3, opts.width))?;
    }
    Ok(())
}

/// The CONTEXT LOAD block: what it measures, one bar per assistant (share of the window,
/// colored against the budget) with its tokens and shares, what always loads, what loads on
/// demand, and what the budget is.
fn context_block<W: Write + ?Sized>(
    w: &mut W,
    report: &Report,
    p: Painter,
    width: Option<usize>,
) -> io::Result<()> {
    let c = &report.context;
    writeln!(w)?;
    writeln!(
        w,
        " {}   {}",
        p.paint(BOLD, "CONTEXT LOAD"),
        wrap(
            &format!("Always-on context: {}", context::MEANING),
            16,
            width
        )
    )?;
    let (window_h, budget_h) = context::share_headers(c);
    // On narrow terminals the bar gives way first.
    let bar = width.map_or(context::BAR, |w| {
        w.saturating_sub(ROW_WITHOUT_BAR)
            .clamp(context::BAR / 2, context::BAR)
    });
    let header = format!(
        "{:<16}{:<bar$}   {:>6}{:>16}{:>14}",
        "", "", "tokens", window_h, budget_h
    );
    writeln!(w, "   {}", p.paint(DIM, header.trim_end()))?;
    for (name, t) in context::rows(c) {
        let style = match context::level(t, c.budget) {
            Level::Ok => GREEN,
            Level::Near => YELLOW,
            Level::Over => RED,
        };
        let filled = context::filled(t, c.window, bar);
        let (of_window, of_budget) = context::shares(c, t);
        writeln!(
            w,
            "   {name:<16}{}{}   {:>6}{:>16}{:>14}",
            p.paint(style, "█".repeat(filled)),
            p.paint(DIM, "░".repeat(bar - filled)),
            tokens(t),
            pct(of_window),
            pct(of_budget)
        )?;
    }
    writeln!(
        w,
        "   {:<16}{}",
        "Always loaded",
        wrap(&context::always_loaded(c, str::to_owned), 19, width)
    )?;
    if c.on_demand_tokens > 0 {
        writeln!(
            w,
            "   {:<16}{}",
            "On demand",
            wrap(&context::on_demand(c), 19, width)
        )?;
    }
    writeln!(
        w,
        "   {:<16}{}",
        "Budget",
        wrap(
            &format!(
                "{}. Bars show the share of the {} context window; they turn yellow above 75% \
                 of the budget and red above it.",
                context::budget(c),
                tokens(c.window)
            ),
            19,
            width
        )
    )?;
    Ok(())
}

/// Width of a CONTEXT LOAD row without its bar: indent 3, name 16, gap 3, then the
/// tokens (6), window (16) and budget (14) columns of the `{:>6}{:>16}{:>14}` formats.
const ROW_WITHOUT_BAR: usize = 3 + 16 + 3 + 6 + 16 + 14;

/// The SCOPE block, with `--assistant` only: who the scan is for and what it left out
/// (listed with `-v`).
fn scope_block<W: Write + ?Sized>(
    w: &mut W,
    report: &Report,
    p: Painter,
    opts: &TerminalOptions<'_>,
) -> io::Result<()> {
    let (Some(scope), Some(left_out)) = (context::scope(report), context::not_loaded(report))
    else {
        return Ok(());
    };
    writeln!(w)?;
    writeln!(w, " {}          {scope}", p.paint(BOLD, "SCOPE"))?;
    let hint = if report.not_loaded.is_empty() || opts.verbose {
        String::new()
    } else {
        " · run with -v to list them".to_owned()
    };
    writeln!(
        w,
        "   {:<16}{}",
        "Not scanned",
        wrap(&format!("{left_out}{hint}"), 19, opts.width)
    )?;
    if opts.verbose {
        let path_w = report
            .not_loaded
            .iter()
            .map(|n| n.rel_path.chars().count())
            .max()
            .unwrap_or(0)
            + 2;
        for n in &report.not_loaded {
            writeln!(
                w,
                "{:19}{:<path_w$}{}",
                "",
                n.rel_path,
                p.paint(DIM, context::not_loaded_reason(n))
            )?;
        }
    }
    Ok(())
}

/// Whether the `↳` fix line of `f` is shown: Medium and higher, or all with `-v`.
fn shows_fix(f: &Finding, opts: &TerminalOptions<'_>) -> bool {
    opts.verbose || f.severity >= Severity::Medium
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
            wrap(&f.message, indent.len() + rule_w, opts.width)
        )?;
        if let Some(fix) = f.remediation.as_deref().filter(|_| shows_fix(f, opts)) {
            writeln!(
                w,
                "{indent}{} {}",
                p.paint(DIM, "↳"),
                wrap(fix, indent.len() + 2, opts.width)
            )?;
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

/// Narrower than this, wrapping would leave a ragged column of single words.
const MIN_WRAP: usize = 30;

/// `text` starting at column `col`, wrapped at spaces to fit `width`; continuation lines
/// are indented to `col`. Words longer than the room stay whole, and a `·` separator stays
/// at the end of its line.
fn wrap(text: &str, col: usize, width: Option<usize>) -> std::borrow::Cow<'_, str> {
    let room = match width {
        Some(w) if w.saturating_sub(col) >= MIN_WRAP => w - col,
        _ => return text.into(),
    };
    if text.chars().count() <= room {
        return text.into();
    }
    let mut out = String::with_capacity(text.len() + 16);
    let mut used = 0;
    let mut words = text.split(' ').peekable();
    while let Some(word) = words.next() {
        let len = word.chars().count();
        // Room for a trailing ` ·` too, so the separator never starts a line.
        let tail = if words.peek() == Some(&"·") { 2 } else { 0 };
        if used > 0 && word != "·" && used + 1 + len + tail > room {
            out.push('\n');
            out.extend(std::iter::repeat_n(' ', col));
            used = 0;
        } else if used > 0 {
            out.push(' ');
            used += 1;
        }
        out.push_str(word);
        used += len;
    }
    out.into()
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
    fn wrap_indents_continuation_lines() {
        let text = "Start the description with when to use it and name the inputs it handles.";
        assert_eq!(
            wrap(text, 4, Some(40)),
            "Start the description with when to\n    use it and name the inputs it\n    handles."
        );
    }

    #[test]
    fn wrap_keeps_short_text_long_words_and_narrow_terminals() {
        assert_eq!(wrap("fits", 4, Some(40)), "fits");
        assert_eq!(wrap("a b", 4, None), "a b");
        let long = "x".repeat(50);
        assert_eq!(
            wrap(&format!("a {long}"), 4, Some(40)),
            format!("a\n    {long}")
        );
        // Less than MIN_WRAP columns of room: one line.
        let text = "one two three four five six seven eight nine ten";
        assert_eq!(wrap(text, 20, Some(40)), text);
    }

    #[test]
    fn wrap_keeps_separators_at_line_ends() {
        let text = "agent.tools.json 15 · tools/fetch.json 14 · CLAUDE.md 6 · …";
        assert_eq!(
            wrap(text, 2, Some(45)),
            "agent.tools.json 15 · tools/fetch.json 14 ·\n  CLAUDE.md 6 · …"
        );
    }

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
