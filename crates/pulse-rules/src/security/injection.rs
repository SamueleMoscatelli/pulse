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
            let phrase = truncate(&inj.display[id], MAX_QUOTE);
            let mut f = Finding::new(
                &META,
                asset,
                span,
                format!("Prompt-injection phrase \"{phrase}\""),
            );
            if guards.covers(span) {
                f.severity = Severity::Low;
            }
            out.push(f);
        }
    }
}

/// Whether `range` in `text` starts and ends on word boundaries (where the phrase has
/// word characters at its edges), so "renew instructions:" doesn't match "new instructions:".
fn at_word_bounds(text: &str, range: Range<usize>) -> bool {
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

/// AI-SEC-002 FP guards: source ranges where matches are downgraded to Low.
struct Guards(Vec<Span>);

impl Guards {
    fn of(asset: &Asset) -> Self {
        if !KindSet::MARKDOWN.contains(asset.kind) {
            return Guards(Vec::new());
        }
        let md = asset.markdown();
        let end = u32::try_from(asset.source.len()).unwrap_or(u32::MAX);
        let mut spans: Vec<Span> = md
            .code_blocks
            .iter()
            .filter(|b| {
                b.lang
                    .as_deref()
                    .is_some_and(|l| FENCE_LANGS.iter().any(|f| l.eq_ignore_ascii_case(f)))
            })
            .map(|b| b.span)
            .collect();
        spans.extend(security_sections(md, end));
        Guards(spans)
    }

    fn covers(&self, span: Span) -> bool {
        self.0
            .iter()
            .any(|g| g.start <= span.start && span.end <= g.end)
    }
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
}
