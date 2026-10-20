//! Small helpers shared by rules.

use pulse_core::text::levenshtein_within;
use pulse_core::{Asset, Finding, RuleMeta, Severity, Span};

/// Hits from which a per-asset summary finding is reported instead of one per hit.
pub const AGGREGATE_MIN: usize = 3;

/// Most distinct quotes listed in a summary finding.
const TOP_QUOTES: usize = 3;
/// Longest quote in a summary finding.
const SUMMARY_QUOTE: usize = 20;

/// One lexicon hit of an aggregating rule.
#[derive(Debug, Clone)]
pub struct Hit<'a> {
    pub span: Span,
    /// The lexicon phrase that matched (hits with equal quotes are counted together).
    pub quote: &'a str,
    pub token_waste: u32,
}

/// Spec AI-EFF-007 aggregation: with ≥ [`AGGREGATE_MIN`] hits, one finding at the first
/// hit, with the rule's severity, the summed waste and `many(count, top quotes)` as message;
/// otherwise one Info finding per hit with `one(hit)` as message.
pub fn aggregate<'a>(
    meta: &RuleMeta,
    asset: &Asset,
    hits: &[Hit<'a>],
    one: impl Fn(&Hit<'a>) -> String,
    many: impl FnOnce(usize, &str) -> String,
    out: &mut Vec<Finding>,
) {
    let Some(first) = hits.first() else {
        return;
    };
    if hits.len() < AGGREGATE_MIN {
        for h in hits {
            let mut f = Finding::new(meta, asset, h.span, one(h));
            f.severity = Severity::Info;
            f.token_waste = h.token_waste;
            out.push(f);
        }
        return;
    }
    // (count, first index, quote), most frequent first, then in source order.
    let mut counts: Vec<(usize, usize, &str)> = Vec::new();
    for (i, h) in hits.iter().enumerate() {
        match counts
            .iter_mut()
            .find(|c| c.2.eq_ignore_ascii_case(h.quote))
        {
            Some(c) => c.0 += 1,
            None => counts.push((1, i, h.quote)),
        }
    }
    counts.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut top: Vec<String> = counts
        .iter()
        .take(TOP_QUOTES)
        .map(|&(n, _, q)| {
            let q = truncate(q, SUMMARY_QUOTE);
            if n > 1 {
                format!("\"{q}\" ×{n}")
            } else {
                format!("\"{q}\"")
            }
        })
        .collect();
    if counts.len() > TOP_QUOTES {
        top.push("…".into());
    }
    let mut f = Finding::new(meta, asset, first.span, many(hits.len(), &top.join(", ")));
    f.token_waste = hits
        .iter()
        .fold(0, |sum: u32, h| sum.saturating_add(h.token_waste));
    out.push(f);
}

/// `s` cut to at most `max` chars, with `…` appended when cut.
pub fn truncate(s: &str, max: usize) -> std::borrow::Cow<'_, str> {
    match s.char_indices().nth(max) {
        None => s.into(),
        Some((i, _)) => {
            let cut = s
                .char_indices()
                .nth(max.saturating_sub(1))
                .map_or(i, |(j, _)| j);
            format!("{}…", s[..cut].trim_end()).into()
        }
    }
}

/// The last `max` chars of `s`, with a leading `…` when cut.
pub fn tail(s: &str, max: usize) -> std::borrow::Cow<'_, str> {
    let n = s.chars().count();
    if n <= max {
        return s.into();
    }
    let start = s.char_indices().nth(n - max + 1).map_or(0, |(i, _)| i);
    format!("…{}", &s[start..]).into()
}

/// Levenshtein distance between `a` and `b` (ASCII case-insensitive) when at most `max`.
pub fn distance_within(a: &str, b: &str, max: usize) -> Option<usize> {
    levenshtein_within(&a.to_ascii_lowercase(), &b.to_ascii_lowercase(), max)
}

/// The closest of `known` to `word` within a third of its length (at least 1, at most 3
/// edits), ignoring ASCII case; ties keep the first in `known`.
pub fn suggest<'a>(word: &str, known: &[&'a str]) -> Option<&'a str> {
    let max = (word.chars().count() / 3).clamp(1, 3);
    known
        .iter()
        .filter_map(|k| Some((distance_within(word, k, max)?, *k)))
        .min_by_key(|&(d, _)| d)
        .map(|(_, k)| k)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_keeps_the_end() {
        assert_eq!(tail("abc", 5), "abc");
        assert_eq!(tail("abcdefgh", 5), "…efgh");
    }

    #[test]
    fn distance_ignores_ascii_case() {
        assert_eq!(distance_within("Reed", "Read", 1), Some(1));
        assert_eq!(distance_within("read", "Read", 0), Some(0));
        assert_eq!(distance_within("kitten", "sitting", 2), None);
    }

    #[test]
    fn suggest_picks_closest_within_budget() {
        let known = ["Read", "Write", "WebFetch"];
        assert_eq!(suggest("Reed", &known), Some("Read"));
        assert_eq!(suggest("WebFech", &known), Some("WebFetch"));
        assert_eq!(suggest("Deploy", &known), None);
    }

    fn agg(quotes: &[&str]) -> Vec<Finding> {
        use pulse_core::{AssetKind, Category, KindSet};
        static META: RuleMeta = RuleMeta {
            id: "AI-EFF-007",
            name: "t",
            category: Category::Efficiency,
            default_severity: Severity::Low,
            kinds: KindSet::MARKDOWN,
            summary: "",
            remediation: "",
            inline_suppressible: true,
        };
        let a = Asset::from_bytes(
            std::path::PathBuf::from("CLAUDE.md"),
            "CLAUDE.md",
            AssetKind::Instructions,
            AssetKind::Instructions.default_load(),
            b"0123456789".to_vec(),
        );
        let hits: Vec<_> = quotes
            .iter()
            .zip(0u32..)
            .map(|(q, i)| Hit {
                span: Span::new(i, i + 1),
                quote: q,
                token_waste: 2,
            })
            .collect();
        let mut out = Vec::new();
        aggregate(
            &META,
            &a,
            &hits,
            |h| format!("one {}", h.quote),
            |n, top| format!("{n}: {top}"),
            &mut out,
        );
        out
    }

    #[test]
    fn aggregate_below_min_is_info_per_hit() {
        let f = agg(&["a", "b"]);
        assert_eq!(f.len(), 2);
        assert!(f.iter().all(|f| f.severity == Severity::Info));
        assert_eq!(f[1].message, "one b");
        assert_eq!(f[1].token_waste, 2);
        assert!(agg(&[]).is_empty());
    }

    #[test]
    fn aggregate_at_min_is_one_summary_with_top_quotes() {
        let f = agg(&["x", "b", "a", "B", "c"]);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].severity, Severity::Low);
        assert_eq!(f[0].message, "5: \"b\" ×2, \"x\", \"a\", …");
        assert_eq!(f[0].token_waste, 10);
        assert_eq!(f[0].location.col, 1);
    }

    #[test]
    fn truncate_counts_chars() {
        assert_eq!(truncate("abc", 3), "abc");
        assert_eq!(truncate("abcd", 3), "ab…");
        assert_eq!(truncate("éééé", 3), "éé…");
        assert_eq!(truncate("", 0), "");
    }
}
