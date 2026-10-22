//! Machine-readable report, schema `pulse/report@1` (spec 06-reports → JSON).

use std::io::{self, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use pulse_core::{Category, Finding, Rating, Severity};
use pulse_engine::{GateStatus, GateValue, Report, category_of};
use serde::Serialize;

use crate::rel_path;

/// The `schema` field; bump on breaking changes.
pub const SCHEMA: &str = "pulse/report@1";

/// How to render a [`Report`] as JSON.
#[derive(Debug, Clone, Copy)]
pub struct JsonOptions<'a> {
    /// Scan root; finding paths are relative to it.
    pub root: &'a Path,
    /// Omit findings below this severity (metrics and gate still count them).
    pub min_severity: Severity,
    /// `meta.generated_at`; tests pass a fixed time.
    pub generated_at: SystemTime,
}

#[derive(Serialize)]
struct JsonReport<'a> {
    schema: &'static str,
    meta: Meta,
    gate: Gate,
    ratings: Ratings,
    metrics: Metrics,
    findings: Vec<JsonFinding<'a>>,
}

#[derive(Serialize)]
struct Meta {
    version: &'static str,
    root: String,
    duration_ms: u64,
    generated_at: String,
}

#[derive(Serialize)]
struct Gate {
    status: &'static str,
    conditions: Vec<Condition>,
}

#[derive(Serialize)]
struct Condition {
    name: &'static str,
    actual: Value,
    threshold: Value,
    passed: bool,
}

/// A gate value: severity and rating as strings, ratios and tokens as numbers, none as null.
#[derive(Serialize)]
#[serde(untagged)]
enum Value {
    Text(&'static str),
    Ratio(f64),
    Tokens(u64),
    None,
}

impl From<GateValue> for Value {
    fn from(v: GateValue) -> Self {
        match v {
            GateValue::NoFindings => Self::None,
            GateValue::Severity(s) => Self::Text(s.as_str()),
            GateValue::Ratio(r) => Self::Ratio(r),
            GateValue::Tokens(t) => Self::Tokens(t),
            GateValue::Rating(r) => Self::Text(r.as_str()),
        }
    }
}

#[derive(Serialize)]
struct Ratings {
    security: Rating,
    reliability: Rating,
    clarity: Rating,
    efficiency: Rating,
}

#[derive(Serialize)]
struct Metrics {
    assets: usize,
    total_tokens: u64,
    always_on_tokens: u64,
    context_load: f64,
    context_debt: f64,
    issues: Issues,
    /// Issues of new findings, in new-code mode only (ratings and gate use these).
    #[serde(skip_serializing_if = "Option::is_none")]
    new_issues: Option<Issues>,
}

#[derive(Serialize)]
struct Issues {
    critical: usize,
    high: usize,
    medium: usize,
    low: usize,
    info: usize,
}

#[derive(Serialize)]
struct JsonFinding<'a> {
    rule: &'static str,
    severity: Severity,
    category: Option<Category>,
    path: String,
    line: u32,
    col: u32,
    end_line: u32,
    end_col: u32,
    message: &'a str,
    remediation: Option<&'static str>,
    token_waste: u32,
    fingerprint: String,
    /// Not in the baseline (always true without one).
    new: bool,
}

/// Writes `report` to `w` as pretty-printed JSON, with a trailing newline.
pub fn render<W: Write + ?Sized>(
    w: &mut W,
    report: &Report,
    opts: &JsonOptions<'_>,
) -> io::Result<()> {
    let m = &report.metrics;
    let issues = |m: &pulse_engine::Metrics| {
        let count = |s| m.issues_by_severity.get(&s).copied().unwrap_or(0);
        Issues {
            critical: count(Severity::Critical),
            high: count(Severity::High),
            medium: count(Severity::Medium),
            low: count(Severity::Low),
            info: count(Severity::Info),
        }
    };
    let r = &report.ratings;
    let doc = JsonReport {
        schema: SCHEMA,
        meta: Meta {
            version: env!("CARGO_PKG_VERSION"),
            root: opts.root.to_string_lossy().replace('\\', "/"),
            duration_ms: u64::try_from(report.timings.total.as_millis()).unwrap_or(u64::MAX),
            generated_at: rfc3339(opts.generated_at),
        },
        gate: Gate {
            status: match report.gate.status {
                GateStatus::Passed => "passed",
                GateStatus::Failed => "failed",
            },
            conditions: report
                .gate
                .conditions
                .iter()
                .map(|c| Condition {
                    name: c.name,
                    actual: c.actual.into(),
                    threshold: c.threshold.into(),
                    passed: c.passed,
                })
                .collect(),
        },
        ratings: Ratings {
            security: r.security,
            reliability: r.reliability,
            clarity: r.clarity,
            efficiency: r.efficiency,
        },
        metrics: Metrics {
            assets: m.assets_by_kind.values().sum(),
            total_tokens: m.total_tokens,
            always_on_tokens: m.always_on_tokens,
            context_load: m.context_load,
            context_debt: m.context_debt,
            issues: issues(m),
            new_issues: report.new_metrics.as_ref().map(issues),
        },
        findings: report
            .findings
            .iter()
            .filter(|f| f.severity >= opts.min_severity)
            .map(|f| finding(f, opts.root, report.is_new(f)))
            .collect(),
    };
    serde_json::to_writer_pretty(&mut *w, &doc)?;
    writeln!(w)
}

fn finding<'a>(f: &'a Finding, root: &Path, new: bool) -> JsonFinding<'a> {
    let l = &f.location;
    JsonFinding {
        rule: f.rule_id,
        severity: f.severity,
        category: category_of(f.rule_id),
        path: rel_path(&l.path, root),
        line: l.line,
        col: l.col,
        end_line: l.end_line,
        end_col: l.end_col,
        message: &f.message,
        remediation: f.remediation,
        token_waste: f.token_waste,
        fingerprint: crate::fingerprint(f),
        new,
    }
}

/// `t` as an RFC 3339 UTC timestamp with second precision (`2026-10-12T10:00:00Z`).
fn rfc3339(t: SystemTime) -> String {
    let secs = t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let (days, rem) = (secs / 86_400, secs % 86_400);
    // Civil date from days since the epoch (H. Hinnant's `civil_from_days`).
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z % 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + u64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3_600,
        rem % 3_600 / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn rfc3339_formats_utc_dates() {
        let at = |s| rfc3339(UNIX_EPOCH + Duration::from_secs(s));
        assert_eq!(at(0), "1970-01-01T00:00:00Z");
        assert_eq!(at(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(at(1_791_799_200), "2026-10-12T10:00:00Z");
        assert_eq!(at(4_107_542_399), "2100-02-28T23:59:59Z");
    }
}
