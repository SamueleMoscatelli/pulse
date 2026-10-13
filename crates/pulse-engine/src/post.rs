//! Post-processing of raw findings: dedup, sort, fingerprints.

use std::cmp::Reverse;
use std::collections::HashMap;
use std::path::Path;

use pulse_core::{Asset, Category, Finding, Location};
use xxhash_rust::xxh3::Xxh3;

/// Rule IDs whose findings win over another rule's on the same span (spec: AI-SEC-003 over
/// AI-SEC-004).
const PREFERRED: &[(&str, &str)] = &[("AI-SEC-003", "AI-SEC-004")];

/// Keeps one rule per (span, category): the rule with the highest-severity finding, ties to
/// the lowest rule ID; all of that rule's findings there are kept.
/// Findings of unregistered (custom) rules are never merged.
pub fn dedup(findings: Vec<Finding>) -> Vec<Finding> {
    let mut groups: HashMap<(Location, Category), Vec<Finding>> = HashMap::new();
    let mut out = Vec::with_capacity(findings.len());
    for f in findings {
        match pulse_rules::find(f.rule_id) {
            Some(meta) => groups
                .entry((f.location.clone(), meta.category))
                .or_default()
                .push(f),
            None => out.push(f),
        }
    }
    for mut group in groups.into_values() {
        prefer(&mut group);
        // Merge across rules only: every finding of the winning rule is kept.
        if let Some(best) = group.iter().map(|f| (f.severity, Reverse(f.rule_id))).max() {
            let winner = best.1.0;
            out.extend(group.into_iter().filter(|f| f.rule_id == winner));
        }
    }
    out
}

/// Drops the losers of [`PREFERRED`] pairs when their winner is in `group`.
fn prefer(group: &mut Vec<Finding>) {
    for (winner, loser) in PREFERRED {
        if group.iter().any(|f| f.rule_id == *winner) {
            group.retain(|f| f.rule_id != *loser);
        }
    }
}

/// Sorts by (path, line, col, rule), then the rest of the location and the message.
pub fn sort(findings: &mut [Finding]) {
    findings.sort_unstable_by(|a, b| {
        let (la, lb) = (&a.location, &b.location);
        (
            &la.path,
            la.line,
            la.col,
            a.rule_id,
            la.end_line,
            la.end_col,
            &a.message,
        )
            .cmp(&(
                &lb.path,
                lb.line,
                lb.col,
                b.rule_id,
                lb.end_line,
                lb.end_col,
                &b.message,
            ))
    });
}

/// Sets each fingerprint (spec 04-scoring-gate → Fingerprints): xxh3 of rule ID, relative path,
/// trimmed text of the first line and the occurrence index of that triple. `findings` must
/// be sorted, so occurrence indices are deterministic.
pub fn fingerprint(findings: &mut [Finding], assets: &[Asset]) {
    let by_path: HashMap<&Path, &Asset> = assets.iter().map(|a| (&*a.path, a)).collect();
    let mut seen: HashMap<(&'static str, String, String), u32> = HashMap::new();
    for f in findings {
        let (rel, text) = match by_path.get(&*f.location.path) {
            Some(a) => {
                let span = a.lines.line_span(&a.source, f.location.line);
                let text = a
                    .source
                    .get(span.start as usize..span.end as usize)
                    .unwrap_or("");
                (a.rel_path.clone(), text.trim().to_owned())
            }
            None => (
                f.location.path.to_string_lossy().replace('\\', "/"),
                String::new(),
            ),
        };
        let mut h = Xxh3::new();
        for part in [f.rule_id, &rel, &text] {
            h.update(part.as_bytes());
            h.update(&[0]);
        }
        let n = seen.entry((f.rule_id, rel, text)).or_insert(0);
        h.update(&n.to_le_bytes());
        *n += 1;
        f.fingerprint = h.digest();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use pulse_core::{AssetKind, LoadMode, Severity, Span};

    fn asset(src: &str) -> Asset {
        Asset::new(
            PathBuf::from("/r/CLAUDE.md"),
            "CLAUDE.md",
            AssetKind::Instructions,
            LoadMode::Always,
            src.to_owned(),
        )
    }

    fn finding(a: &Asset, rule_id: &'static str, sev: Severity, span: Span) -> Finding {
        Finding {
            rule_id,
            severity: sev,
            location: a.span_to_location(span),
            message: String::new(),
            remediation: None,
            token_waste: 0,
            fingerprint: 0,
        }
    }

    #[test]
    fn dedup_keeps_highest_severity_per_span_and_category() {
        let a = asset("one\ntwo\n");
        let s = Span::new(0, 3);
        let out = dedup(vec![
            finding(&a, "AI-STR-002", Severity::Low, s),
            finding(&a, "AI-STR-002", Severity::High, s),
            finding(&a, "AI-STR-002", Severity::Low, Span::new(4, 7)),
            finding(&a, "X-ACME-001", Severity::Low, s),
            finding(&a, "X-ACME-002", Severity::Low, s),
        ]);
        assert_eq!(out.len(), 5, "same-rule findings are not merged");
        assert!(
            out.iter()
                .any(|f| f.rule_id == "AI-STR-002" && f.severity == Severity::High)
        );
    }

    #[test]
    fn prefer_drops_sec004_next_to_sec003() {
        let a = asset("x");
        let s = Span::new(0, 1);
        let mut g = vec![
            finding(&a, "AI-SEC-004", Severity::Critical, s),
            finding(&a, "AI-SEC-003", Severity::High, s),
        ];
        prefer(&mut g);
        assert_eq!(
            g.iter().map(|f| f.rule_id).collect::<Vec<_>>(),
            ["AI-SEC-003"]
        );
        let mut alone = vec![finding(&a, "AI-SEC-004", Severity::Critical, s)];
        prefer(&mut alone);
        assert_eq!(alone.len(), 1);
    }

    #[test]
    fn fingerprints_survive_line_moves_and_count_occurrences() {
        let fp = |src: &str, lines: &[u32]| {
            let a = asset(src);
            let mut f: Vec<Finding> = lines
                .iter()
                .map(|&l| {
                    let span = a.lines.line_span(&a.source, l);
                    finding(&a, "AI-STR-002", Severity::High, span)
                })
                .collect();
            sort(&mut f);
            fingerprint(&mut f, std::slice::from_ref(&a));
            f.iter().map(|f| f.fingerprint).collect::<Vec<_>>()
        };
        let before = fp("dup\nbad line\ndup\n", &[1, 2, 3]);
        let after = fp("new\n\ndup\n  bad line\ndup\n", &[3, 4, 5]);
        assert_eq!(before.len(), 3);
        assert_ne!(
            before[0], before[2],
            "occurrence index separates identical lines"
        );
        let (mut b, mut a) = (before.clone(), after.clone());
        b.sort_unstable();
        a.sort_unstable();
        assert_eq!(b, a);
    }
}
