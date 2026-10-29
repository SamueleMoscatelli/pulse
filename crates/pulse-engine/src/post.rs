//! Post-processing of raw findings: dedup, sort, fingerprints.

use std::cmp::{Ordering, Reverse};
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
    // Grouped by index, borrowing locations: no `Location` clone per finding (P7.1).
    let mut groups: HashMap<(&Location, Category), Vec<usize>> = HashMap::new();
    let mut keep = vec![false; findings.len()];
    for (i, f) in findings.iter().enumerate() {
        match pulse_rules::find(f.rule_id) {
            Some(meta) => groups
                .entry((&f.location, meta.category))
                .or_default()
                .push(i),
            None => keep[i] = true,
        }
    }
    for mut group in groups.into_values() {
        prefer(&mut group, &findings);
        // Merge across rules only: every finding of the winning rule is kept.
        let best = group
            .iter()
            .map(|&i| (findings[i].severity, Reverse(findings[i].rule_id)))
            .max();
        if let Some((_, Reverse(winner))) = best {
            for i in group {
                keep[i] |= findings[i].rule_id == winner;
            }
        }
    }
    findings
        .into_iter()
        .zip(keep)
        .filter_map(|(f, k)| k.then_some(f))
        .collect()
}

/// Drops the losers of [`PREFERRED`] pairs when their winner is in `group`.
fn prefer(group: &mut Vec<usize>, findings: &[Finding]) {
    for (winner, loser) in PREFERRED {
        if group.iter().any(|&i| findings[i].rule_id == *winner) {
            group.retain(|&i| findings[i].rule_id != *loser);
        }
    }
}

/// Sorts by (path, line, col, rule), then the rest of the location and the message.
pub fn sort(findings: &mut [Finding]) {
    findings.sort_unstable_by(|a, b| {
        let (la, lb) = (&a.location, &b.location);
        // Byte-equal paths first: `Path::cmp` walks components, and most comparisons are
        // within one file (P7.1).
        let path = if la.path.as_os_str() == lb.path.as_os_str() {
            Ordering::Equal
        } else {
            la.path.cmp(&lb.path)
        };
        path.then_with(|| {
            (
                la.line,
                la.col,
                a.rule_id,
                la.end_line,
                la.end_col,
                &a.message,
            )
                .cmp(&(
                    lb.line,
                    lb.col,
                    b.rule_id,
                    lb.end_line,
                    lb.end_col,
                    &b.message,
                ))
        })
    });
}

/// Sets each fingerprint (spec 04-scoring-gate → Fingerprints): xxh3 of rule ID, relative path,
/// trimmed text of the first line and the occurrence index of that triple. `findings` must
/// be sorted, so occurrence indices are deterministic.
pub fn fingerprint(findings: &mut [Finding], assets: &[Asset], root: &Path) {
    let by_path: HashMap<&Path, &Asset> = assets.iter().map(|a| (&*a.path, a)).collect();
    // Keyed by the digest of `rule\0rel\0text\0`, and that hasher state memoized per
    // (rule, path, line): hashing or copying the line once per finding is quadratic over
    // many findings on one huge line (P7.1). Fingerprint values are unchanged.
    let mut seen: HashMap<u64, u32> = HashMap::new();
    // Findings are sorted by (path, line, …), so the memo only holds the current line.
    let mut line: Option<(&Path, u32)> = None;
    let mut prefix: HashMap<&'static str, (Xxh3, u64)> = HashMap::new();
    for f in findings.iter_mut() {
        let here = (&*f.location.path, f.location.line);
        if line != Some(here) {
            prefix.clear();
            line = Some(here);
        }
        let (base, digest) = prefix.entry(f.rule_id).or_insert_with(|| {
            let (rel, text) = match by_path.get(&*f.location.path) {
                Some(a) => {
                    let span = a.lines.line_span(&a.source, f.location.line);
                    let text = a
                        .source
                        .get(span.start as usize..span.end as usize)
                        .unwrap_or("");
                    (a.rel_path.clone(), text.trim())
                }
                // A file that is not an asset (AI-STR-008): its root-relative path, so the
                // fingerprint does not depend on where the repo is checked out.
                None => {
                    let path = f
                        .location
                        .path
                        .strip_prefix(root)
                        .unwrap_or(&f.location.path);
                    (path.to_string_lossy().replace('\\', "/"), "")
                }
            };
            let mut h = Xxh3::new();
            for part in [f.rule_id, &rel, text] {
                h.update(part.as_bytes());
                h.update(&[0]);
            }
            let digest = h.digest();
            (h, digest)
        });
        let n = seen.entry(*digest).or_insert(0);
        let mut h = base.clone();
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
        let fs = vec![
            finding(&a, "AI-SEC-004", Severity::Critical, s),
            finding(&a, "AI-SEC-003", Severity::High, s),
        ];
        let mut g = vec![0, 1];
        prefer(&mut g, &fs);
        assert_eq!(g, [1]);
        let mut alone = vec![0];
        prefer(&mut alone, &fs);
        assert_eq!(alone, [0]);
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
            fingerprint(&mut f, std::slice::from_ref(&a), Path::new(""));
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
