//! Quality Gate evaluation (spec 04-scoring-gate → Quality Gate).

use std::fmt;

use pulse_core::config::Gate as GateConfig;
use pulse_core::{Finding, Rating, Severity};

use crate::score::{Metrics, Ratings};

/// Overall gate outcome.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GateStatus {
    #[default]
    Passed,
    Failed,
}

/// A value compared by a gate condition.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GateValue {
    /// No open finding (the actual side of `fail_on_severity`).
    NoFindings,
    Severity(Severity),
    /// A fraction in [0, 1], shown as a percentage.
    Ratio(f64),
    Tokens(u64),
    Rating(Rating),
}

impl fmt::Display for GateValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoFindings => f.write_str("none"),
            Self::Severity(s) => s.fmt(f),
            Self::Ratio(r) => write!(f, "{:.1}%", r * 100.0),
            Self::Tokens(t) => t.fmt(f),
            Self::Rating(r) => r.fmt(f),
        }
    }
}

/// One evaluated gate condition.
#[derive(Debug, Clone, PartialEq)]
pub struct Condition {
    /// The config key, e.g. `max_context_debt` or `min_ratings.security`.
    pub name: &'static str,
    pub actual: GateValue,
    pub threshold: GateValue,
    pub passed: bool,
}

/// Gate result: every enabled condition, passed or failed.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Gate {
    pub status: GateStatus,
    pub conditions: Vec<Condition>,
}

/// Evaluates the enabled conditions of `cfg`; disabled ones (`"none"`, `0`, unset rating)
/// are not listed.
pub fn evaluate(cfg: &GateConfig, findings: &[Finding], m: &Metrics, r: &Ratings) -> Gate {
    let mut conditions = Vec::new();
    if let Some(min) = cfg.fail_on_severity {
        let worst = findings.iter().map(|f| f.severity).max();
        conditions.push(Condition {
            name: "fail_on_severity",
            actual: worst.map_or(GateValue::NoFindings, GateValue::Severity),
            threshold: GateValue::Severity(min),
            passed: worst.is_none_or(|w| w < min),
        });
    }
    conditions.push(Condition {
        name: "max_context_debt",
        actual: GateValue::Ratio(m.context_debt),
        threshold: GateValue::Ratio(cfg.max_context_debt),
        passed: m.context_debt <= cfg.max_context_debt,
    });
    if cfg.max_always_on_tokens > 0 {
        conditions.push(Condition {
            name: "max_always_on_tokens",
            actual: GateValue::Tokens(m.always_on_tokens),
            threshold: GateValue::Tokens(cfg.max_always_on_tokens),
            passed: m.always_on_tokens <= cfg.max_always_on_tokens,
        });
    }
    let min = &cfg.min_ratings;
    let dims = [
        ("min_ratings.security", min.security, r.security),
        ("min_ratings.reliability", min.reliability, r.reliability),
        ("min_ratings.clarity", min.clarity, r.clarity),
        ("min_ratings.efficiency", min.efficiency, r.efficiency),
    ];
    for (name, threshold, actual) in dims {
        if let Some(threshold) = threshold {
            conditions.push(Condition {
                name,
                actual: GateValue::Rating(actual),
                threshold: GateValue::Rating(threshold),
                // A is best: the actual rating must be no worse than the minimum.
                passed: actual <= threshold,
            });
        }
    }
    let status = if conditions.iter().all(|c| c.passed) {
        GateStatus::Passed
    } else {
        GateStatus::Failed
    };
    Gate { status, conditions }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Arc;

    use pulse_core::Location;
    use pulse_core::config::MinRatings;

    use super::*;

    fn finding(sev: Severity) -> Finding {
        Finding {
            rule_id: "AI-SEC-001",
            severity: sev,
            location: Location {
                path: Arc::from(Path::new("a.md")),
                line: 1,
                col: 1,
                end_line: 1,
                end_col: 2,
            },
            message: String::new(),
            remediation: None,
            token_waste: 0,
            fingerprint: 0,
        }
    }

    fn only(gate: &Gate, name: &str) -> Condition {
        let found: Vec<_> = gate.conditions.iter().filter(|c| c.name == name).collect();
        assert_eq!(found.len(), 1, "{name}: {:?}", gate.conditions);
        found[0].clone()
    }

    #[test]
    fn default_gate_lists_severity_and_debt() {
        let g = evaluate(
            &GateConfig::default(),
            &[],
            &Metrics::default(),
            &Ratings::default(),
        );
        let names: Vec<_> = g.conditions.iter().map(|c| c.name).collect();
        assert_eq!(names, ["fail_on_severity", "max_context_debt"]);
        assert_eq!(g.status, GateStatus::Passed);
        assert_eq!(g.conditions[0].actual, GateValue::NoFindings);
    }

    #[test]
    fn fail_on_severity_threshold() {
        let cfg = GateConfig::default(); // high
        let cases = [
            (vec![], true),
            (vec![Severity::Info, Severity::Medium], true),
            (vec![Severity::Low, Severity::High], false),
            (vec![Severity::Critical], false),
        ];
        for (sevs, pass) in cases {
            let findings: Vec<_> = sevs.iter().copied().map(finding).collect();
            let g = evaluate(&cfg, &findings, &Metrics::default(), &Ratings::default());
            assert_eq!(only(&g, "fail_on_severity").passed, pass, "{sevs:?}");
            let status = if pass {
                GateStatus::Passed
            } else {
                GateStatus::Failed
            };
            assert_eq!(g.status, status, "{sevs:?}");
        }
    }

    #[test]
    fn fail_on_severity_none_disables() {
        let cfg = GateConfig {
            fail_on_severity: None,
            ..GateConfig::default()
        };
        let g = evaluate(
            &cfg,
            &[finding(Severity::Critical)],
            &Metrics::default(),
            &Ratings::default(),
        );
        assert!(g.conditions.iter().all(|c| c.name != "fail_on_severity"));
        assert_eq!(g.status, GateStatus::Passed);
    }

    #[test]
    fn max_context_debt_threshold() {
        for (debt, pass) in [(0.0, true), (0.10, true), (0.100_001, false), (1.0, false)] {
            let m = Metrics {
                context_debt: debt,
                ..Metrics::default()
            };
            let g = evaluate(&GateConfig::default(), &[], &m, &Ratings::default());
            assert_eq!(only(&g, "max_context_debt").passed, pass, "{debt}");
            assert_eq!(g.status == GateStatus::Passed, pass, "{debt}");
        }
    }

    #[test]
    fn max_always_on_tokens_threshold() {
        let cfg = GateConfig {
            max_always_on_tokens: 1000,
            ..GateConfig::default()
        };
        for (tokens, pass) in [(0, true), (1000, true), (1001, false)] {
            let m = Metrics {
                always_on_tokens: tokens,
                ..Metrics::default()
            };
            let g = evaluate(&cfg, &[], &m, &Ratings::default());
            let c = only(&g, "max_always_on_tokens");
            assert_eq!(c.passed, pass, "{tokens}");
            assert_eq!(c.threshold, GateValue::Tokens(1000));
            assert_eq!(g.status == GateStatus::Passed, pass, "{tokens}");
        }
        // 0 disables the condition.
        let m = Metrics {
            always_on_tokens: u64::MAX,
            ..Metrics::default()
        };
        let g = evaluate(&GateConfig::default(), &[], &m, &Ratings::default());
        assert!(
            g.conditions
                .iter()
                .all(|c| c.name != "max_always_on_tokens")
        );
    }

    #[test]
    fn min_ratings_per_dimension() {
        let cfg = GateConfig {
            min_ratings: MinRatings {
                security: Some(Rating::A),
                reliability: Some(Rating::B),
                clarity: Some(Rating::C),
                efficiency: Some(Rating::D),
            },
            ..GateConfig::default()
        };
        let r = Ratings {
            security: Rating::B,
            reliability: Rating::B,
            clarity: Rating::A,
            efficiency: Rating::E,
        };
        let g = evaluate(&cfg, &[], &Metrics::default(), &r);
        let cases = [
            ("min_ratings.security", false),
            ("min_ratings.reliability", true),
            ("min_ratings.clarity", true),
            ("min_ratings.efficiency", false),
        ];
        for (name, pass) in cases {
            assert_eq!(only(&g, name).passed, pass, "{name}");
        }
        assert_eq!(g.status, GateStatus::Failed);
        // Unset dimensions are not listed.
        let g = evaluate(
            &GateConfig::default(),
            &[],
            &Metrics::default(),
            &Ratings::default(),
        );
        assert!(
            g.conditions
                .iter()
                .all(|c| !c.name.starts_with("min_ratings"))
        );
    }

    #[test]
    fn gate_value_display() {
        assert_eq!(GateValue::Ratio(0.1).to_string(), "10.0%");
        assert_eq!(GateValue::Severity(Severity::High).to_string(), "high");
        assert_eq!(GateValue::Rating(Rating::C).to_string(), "C");
        assert_eq!(GateValue::Tokens(42).to_string(), "42");
        assert_eq!(GateValue::NoFindings.to_string(), "none");
    }
}
