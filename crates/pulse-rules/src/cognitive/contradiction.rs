//! AI-COG-001: contradictory directives within one file.

use pulse_core::{
    Asset, Category, ConfigError, Finding, KindSet, Rule, RuleCtx, RuleMeta, Severity, Span,
};
use serde::Deserialize;

use super::directives::{self, Directive, Polarity, contains_phrase, is, lemma, same, words};
use crate::lexicon::directives::{EXCLUSIVE_SETS, GENERIC_HEADINGS, STOP};
use crate::util::truncate;

pub static META: RuleMeta = RuleMeta {
    id: "AI-COG-001",
    name: "Contradictory directives",
    category: Category::Cognitive,
    default_severity: Severity::Medium,
    kinds: KindSet::MARKDOWN,
    summary: "Two directives in the file contradict each other.",
    remediation: "Remove one directive, or scope both (\"for tests…\", or under headings naming each scope).",
    inline_suppressible: true,
};

/// Longest directive quoted in a finding (two quotes fit in 100 chars).
const MAX_QUOTE: usize = 34;

/// `[rules.AI-COG-001]` options.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Options {
    /// Extra sets of mutually exclusive choices, e.g. `[["bun", "deno"]]`.
    pub exclusive: Vec<Vec<String>>,
}

pub struct ContradictoryDirectives;

impl Rule for ContradictoryDirectives {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn validate(&self, ctx: &RuleCtx<'_>) -> Result<(), ConfigError> {
        ctx.options::<Options>(META.id).map(drop)
    }

    fn check(&self, asset: &Asset, ctx: &RuleCtx<'_>, out: &mut Vec<Finding>) {
        let ds = directives::extract(asset);
        if ds.len() < 2 {
            return;
        }
        let opts = ctx.options::<Options>(META.id).unwrap_or_default();
        let ps: Vec<_> = ds
            .iter()
            .map(|d| Prepared::new(d, &opts.exclusive))
            .collect();
        for (j, b) in ps.iter().enumerate().skip(1) {
            let Some(a) = ps[..j].iter().find(|a| conflict(a, b)) else {
                continue;
            };
            let (a, b) = (a.directive, b.directive);
            let line = asset.span_to_location(a.span).line;
            let msg = format!(
                "Contradicts line {line}: \"{}\" vs \"{}\"",
                quote(asset, a.span),
                quote(asset, b.span)
            );
            out.push(Finding::new(&META, asset, b.span, msg));
        }
    }
}

/// A directive with the per-directive facts [`conflict`] needs, computed once so the
/// pairwise loop stays cheap.
pub struct Prepared<'d, 'a> {
    pub directive: &'d Directive<'a>,
    /// The section heading names a scope (`Python`, not `Code style`).
    scoped_section: bool,
    /// `(set, member)` of every exclusive-set member the object names (positive only);
    /// built-in sets first, then `extra` sets offset by `EXCLUSIVE_SETS.len()`.
    members: Vec<(usize, usize)>,
}

impl<'d, 'a> Prepared<'d, 'a> {
    /// Prepares `directive`, with `extra` exclusive sets from the options.
    pub fn new(directive: &'d Directive<'a>, extra: &[Vec<String>]) -> Self {
        let mut members = Vec::new();
        if directive.polarity == Polarity::Positive {
            let obj = &directive.object;
            for (si, set) in EXCLUSIVE_SETS.iter().enumerate() {
                for (mi, m) in set.iter().enumerate() {
                    if contains_phrase(obj, m) {
                        members.push((si, mi));
                    }
                }
            }
            for (si, set) in extra.iter().enumerate() {
                for (mi, m) in set.iter().enumerate() {
                    if contains_phrase(obj, m) {
                        members.push((EXCLUSIVE_SETS.len() + si, mi));
                    }
                }
            }
        }
        Self {
            directive,
            scoped_section: directive.section.is_some_and(names_scope),
            members,
        }
    }
}

/// Whether `a` and `b` contradict each other (spec AI-COG-001 → Detect 2–3, FP guards).
pub fn conflict(pa: &Prepared<'_, '_>, pb: &Prepared<'_, '_>) -> bool {
    let (a, b) = (pa.directive, pb.directive);
    let hit = match (a.polarity, b.polarity) {
        (Polarity::Positive, Polarity::Positive) => exclusive(&pa.members, &pb.members),
        (Polarity::Negative, Polarity::Negative) => false,
        (Polarity::Positive, Polarity::Negative) => {
            same(a.verb, b.verb) && narrows(&a.object, &b.object)
        }
        (Polarity::Negative, Polarity::Positive) => {
            same(a.verb, b.verb) && narrows(&b.object, &a.object)
        }
    };
    // AI-COG-001 FP guards: scoped qualifiers with different contexts, different scoped sections.
    let scoped_apart = pa.scoped_section
        && pb.scoped_section
        && !a
            .section
            .unwrap_or("")
            .eq_ignore_ascii_case(b.section.unwrap_or(""));
    hit && same_words(&a.context, &b.context) && !scoped_apart
}

/// Whether `a` names one member of a set and `b` another, and neither names the other's.
fn exclusive(a: &[(usize, usize)], b: &[(usize, usize)]) -> bool {
    a.iter().any(|&(sa, x)| {
        b.iter()
            .any(|&(sb, y)| sa == sb && x != y && !a.contains(&(sa, y)) && !b.contains(&(sb, x)))
    })
}

/// Whether the negative object `neg` is a prefix of the positive one `pos`: "Never use
/// semicolons" denies "Always use semicolons at line end", but "Never use TypeScript enums"
/// only narrows "Always use TypeScript".
fn narrows(pos: &[&str], neg: &[&str]) -> bool {
    !neg.is_empty() && neg.len() <= pos.len() && same_words(&pos[..neg.len()], neg)
}

fn same_words(a: &[&str], b: &[&str]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| same(x, y))
}

/// Whether a heading names a scope: some word is not generic (`Code style`, `Rules`).
fn names_scope(heading: &str) -> bool {
    let mut ws = Vec::new();
    words(heading, &mut ws);
    ws.iter().any(|w| {
        let l = lemma(w.text);
        !is(l, GENERIC_HEADINGS) && !is(l, STOP) && !is(w.text, GENERIC_HEADINGS)
    })
}

/// The directive's text on one line, without trailing punctuation, truncated.
fn quote(asset: &Asset, span: Span) -> String {
    let raw = asset
        .source
        .get(span.start as usize..span.end as usize)
        .unwrap_or("");
    let text = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    let text = text.trim_end_matches(['.', '!', '?', ';', ':', ',']);
    truncate(text, MAX_QUOTE).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_headings() {
        assert!(names_scope("Python"));
        assert!(names_scope("Frontend rules"));
        assert!(!names_scope("Code style"));
        assert!(!names_scope("General Rules"));
        assert!(!names_scope("Do and Don't"));
    }

    #[test]
    fn exclusive_needs_distinct_members() {
        // Set 0 = {npm, yarn, pnpm}, set 1 = {bun, deno}.
        assert!(exclusive(&[(0, 0)], &[(0, 2)]));
        assert!(!exclusive(&[(0, 0)], &[(0, 0)]));
        assert!(!exclusive(&[(0, 0), (0, 1)], &[(0, 1)]));
        assert!(!exclusive(&[(1, 0)], &[(0, 1)]));
    }

    #[test]
    fn negative_object_must_not_be_more_specific() {
        assert!(narrows(&["semicolon", "end"], &["Semicolon"]));
        assert!(narrows(&["semicolon"], &["semicolon"]));
        assert!(!narrows(&["typescript"], &["typescript", "enum"]));
        assert!(!narrows(&["typescript", "enum"], &["typescript", "strict"]));
    }
}
