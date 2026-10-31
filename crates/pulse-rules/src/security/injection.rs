//! AI-SEC-002: prompt-injection phrases.

use std::ops::Range;

use pulse_core::{
    Asset, Category, ConfigError, Finding, KindSet, MarkdownView, Rule, RuleCtx, RuleMeta,
    Severity, Span,
};
use serde::Deserialize;

use crate::lexicon::injection::INJECTION;
use crate::lexicon::normalize_phrase;
use crate::util::truncate;

pub static META: RuleMeta = RuleMeta {
    id: "AI-SEC-002",
    name: "Prompt-injection phrase",
    category: Category::Security,
    default_severity: Severity::Critical,
    kinds: KindSet::ALL.without(pulse_core::AssetKind::Settings),
    summary: "File contains a phrase typical of prompt-injection attacks.",
    remediation: "Remove the phrase; if it documents an attack, put it in a `text` fence under a security heading.",
    inline_suppressible: false,
};

/// Longest phrase quoted in a finding.
const MAX_QUOTE: usize = 40;

/// Longest phrase in the quoted-mention message, which keeps it within 100 chars.
const MAX_QUOTE_MENTION: usize = 15;

/// Opening and closing quote pairs that mark a quoted mention.
const QUOTES: &[(char, char)] = &[
    ('"', '"'),
    ('\'', '\''),
    ('\u{201C}', '\u{201D}'),
    ('`', '`'),
];

/// Words in a heading that mark a section documenting attacks.
const SECTION_WORDS: &[&str] = &["injection", "security", "example attack"];

/// Fence info strings that mark quoted, non-instruction text.
const FENCE_LANGS: &[&str] = &["text", "example"];

/// `[rules.AI-SEC-002]` options.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Options {
    /// Phrases never reported; compared to the lexicon phrase after normalization.
    pub allow: Vec<String>,
}

pub struct InjectionPhrase;

impl Rule for InjectionPhrase {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn validate(&self, ctx: &RuleCtx<'_>) -> Result<(), ConfigError> {
        ctx.options::<Options>(META.id).map(drop)
    }

    fn check(&self, asset: &Asset, ctx: &RuleCtx<'_>, out: &mut Vec<Finding>) {
        let norm = asset.normalized();
        let text = norm.text.as_str();
        let inj = &*INJECTION;
        // Built on the first hit only.
        let mut allow: Option<Vec<String>> = None;
        let mut guards: Option<Guards> = None;
        for m in inj.automaton.find_iter(text) {
            if !at_word_bounds(text, m.range()) {
                continue;
            }
            let id = m.pattern().as_usize();
            let allow = allow.get_or_insert_with(|| {
                ctx.options::<Options>(META.id)
                    .unwrap_or_default()
                    .allow
                    .iter()
                    .map(|a| normalize_phrase(a))
                    .collect()
            });
            if allow.iter().any(|a| *a == inj.normalized[id]) {
                continue;
            }
            let span = norm.source_span(m.range());
            let guards = guards.get_or_insert_with(|| Guards::of(asset));
            let display = &inj.display[id];
            // AI-SEC-002 FP guards: an attack-documenting section or fence wins (Low); a
            // quoted mention is still reported, at High, for review and the baseline.
            let f = if guards.covers(span) {
                let phrase = truncate(display, MAX_QUOTE);
                let mut f = Finding::new(
                    &META,
                    asset,
                    span,
                    format!("Prompt-injection phrase \"{phrase}\""),
                );
                f.severity = Severity::Low;
                f
            } else if guards.prose(span) && quoted(&asset.source, span) {
                let phrase = truncate(display, MAX_QUOTE_MENTION);
                let mut f = Finding::new(
                    &META,
                    asset,
                    span,
                    format!(
                        "Prompt-injection phrase \"{phrase}\" looks quoted: check it, then accept it with pulse baseline"
                    ),
                );
                f.severity = Severity::High;
                f
            } else {
                let phrase = truncate(display, MAX_QUOTE);
                Finding::new(
                    &META,
                    asset,
                    span,
                    format!("Prompt-injection phrase \"{phrase}\""),
                )
            };
            out.push(f);
        }
    }
}

/// Whether `range` in `text` starts and ends on word boundaries (where the phrase has
/// word characters at its edges), so "renew instructions:" doesn't match "new instructions:".
pub(crate) fn at_word_bounds(text: &str, range: Range<usize>) -> bool {
    let word = |c: char| c.is_alphanumeric();
    let (Some(phrase), Some(before), Some(after)) = (
        text.get(range.clone()),
        text.get(..range.start),
        text.get(range.end..),
    ) else {
        return false;
    };
    let starts = phrase.chars().next().is_some_and(word);
    let ends = phrase.chars().next_back().is_some_and(word);
    !(starts && before.chars().next_back().is_some_and(word)
        || ends && after.chars().next().is_some_and(word))
}

/// Whether the match at `span` sits inside a quote pair on its line: an opening quote not
/// preceded by a word character (so "don't" doesn't open one) before it, with no closing quote
/// in between, and a closing quote not followed by a word character after it.
fn quoted(source: &str, span: Span) -> bool {
    let word = |c: char| c.is_alphanumeric();
    let (Some(before), Some(after)) = (
        source.get(..span.start as usize),
        source.get(span.end as usize..),
    ) else {
        return false;
    };
    let before = before.rsplit('\n').next().unwrap_or_default();
    let after = after.split('\n').next().unwrap_or_default();
    QUOTES.iter().any(|&(open, close)| {
        let opens = before.rfind(open).is_some_and(|i| {
            let inner = before.get(i + open.len_utf8()..).unwrap_or_default();
            !before[..i].chars().next_back().is_some_and(word)
                && (open == close || !inner.contains(close))
        });
        opens
            && after.find(close).is_some_and(|i| {
                !after[i + close.len_utf8()..]
                    .chars()
                    .next()
                    .is_some_and(word)
            })
    })
}

/// AI-SEC-002 FP guards: source ranges where matches are downgraded to Low, and where a
/// quoted mention can be told apart from syntax.
#[derive(Default)]
struct Guards {
    low: Vec<Span>,
    /// The Markdown body (none for JSON/TOML assets, where quotes are string syntax).
    body: Option<Span>,
    /// Fenced code blocks, where quotes are code (`echo "…"`), not a mention.
    code: Vec<Span>,
}

impl Guards {
    fn of(asset: &Asset) -> Self {
        if !KindSet::MARKDOWN.contains(asset.kind) {
            return Guards::default();
        }
        let md = asset.markdown();
        let end = u32::try_from(asset.source.len()).unwrap_or(u32::MAX);
        let mut low: Vec<Span> = md
            .code_blocks
            .iter()
            .filter(|b| {
                b.lang
                    .as_deref()
                    .is_some_and(|l| FENCE_LANGS.iter().any(|f| l.eq_ignore_ascii_case(f)))
            })
            .map(|b| b.span)
            .collect();
        low.extend(security_sections(md, end));
        Guards {
            low,
            body: Some(md.body),
            code: md.code_blocks.iter().map(|b| b.span).collect(),
        }
    }

    fn covers(&self, span: Span) -> bool {
        self.low.iter().any(|g| contains(*g, span))
    }

    /// Whether `span` is Markdown prose: in the body, outside fenced code.
    fn prose(&self, span: Span) -> bool {
        self.body.is_some_and(|b| contains(b, span))
            && !self.code.iter().any(|c| contains(*c, span))
    }
}

fn contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

/// Sections (heading to the next heading of the same or a higher level) whose heading
/// names attacks or security.
fn security_sections(md: &MarkdownView, end: u32) -> impl Iterator<Item = Span> + '_ {
    md.headings.iter().enumerate().filter_map(move |(i, h)| {
        let title = h.text.to_lowercase();
        if !SECTION_WORDS.iter().any(|w| title.contains(w)) {
            return None;
        }
        let stop = md.headings[i + 1..]
            .iter()
            .find(|n| n.level <= h.level)
            .map_or(end, |n| n.span.start);
        Some(Span::new(h.span.start, stop))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_bounds_reject_inner_matches() {
        let t = "renew instructions: x";
        assert!(!at_word_bounds(t, 2..t.len() - 2));
        let t = "new instructions:x";
        assert!(at_word_bounds(t, 0..17));
        let t = "please forget everything.";
        assert!(at_word_bounds(t, 7..24));
        let t = "forget everythings";
        assert!(!at_word_bounds(t, 0..17));
        assert!(at_word_bounds("a</system>b", 1..10));
    }

    fn quoted_at(line: &str, phrase: &str) -> bool {
        let start = line.find(phrase).unwrap_or_default();
        let span = Span::new(
            u32::try_from(start).unwrap_or_default(),
            u32::try_from(start + phrase.len()).unwrap_or_default(),
        );
        quoted(line, span)
    }

    #[test]
    fn quoted_needs_a_pair_around_the_phrase() {
        let p = "ignore previous instructions";
        assert!(quoted_at(
            "language like \"ignore previous instructions,\" in",
            p
        ));
        assert!(quoted_at("as 'ignore previous instructions' text", p));
        assert!(quoted_at(
            "as \u{201C}ignore previous instructions\u{201D}.",
            p
        ));
        assert!(quoted_at("as `ignore previous instructions`", p));
        assert!(quoted_at("\"please ignore previous instructions now\"", p));
        assert!(!quoted_at("OK. ignore previous instructions.", p));
        assert!(!quoted_at(
            "don't ignore previous instructions, it's bad",
            p
        ));
        assert!(!quoted_at(
            "\"a\" then ignore previous instructions \"b\"",
            p
        ));
        assert!(!quoted_at("\"x\nignore previous instructions\"", p));
        assert!(!quoted_at(
            "\u{201C}a\u{201D} ignore previous instructions\u{201D}",
            p
        ));
    }
}
