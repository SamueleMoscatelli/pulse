//! Directive style: vague directives (AI-COG-002), negative-only constraints (AI-COG-003)
//! and emphasis inflation (AI-COG-004).

use std::sync::LazyLock;

use pulse_core::{Asset, Category, Finding, KindSet, Rule, RuleCtx, RuleMeta, Severity, Span};

use super::directives::{Polarity, Word, extract, is, is_directive, lemma};
use crate::lexicon::directives::{ALTERNATIVES, NEGATIVE_FIRST, NEGATORS, QUALIFIERS};
use crate::lexicon::imperatives::IMPERATIVES;
use crate::lexicon::style::{ACRONYMS, ANCHORS, DIRECTIVE_VERBS, EMPHASIS, VAGUE};
use crate::util::{Hit, aggregate, truncate};

pub static META_VAGUE: RuleMeta = RuleMeta {
    id: "AI-COG-002",
    name: "Vague directive",
    category: Category::Cognitive,
    default_severity: Severity::Low,
    kinds: KindSet::MARKDOWN,
    summary: "A directive uses a vague term with nothing concrete to pin it down.",
    remediation: "Say exactly what you mean: a number, a command, a path or an example.",
    inline_suppressible: true,
};

pub struct VagueDirective;

impl Rule for VagueDirective {
    fn meta(&self) -> &'static RuleMeta {
        &META_VAGUE
    }

    fn check(&self, asset: &Asset, _ctx: &RuleCtx<'_>, out: &mut Vec<Finding>) {
        let src = asset.source.as_str();
        let mut ws = Vec::new();
        let mut hits = Vec::new();
        for s in asset.prose().sentences() {
            let r = s.range.clone();
            let Some(text) = src.get(r.clone()) else {
                continue;
            };
            s.words(src, &mut ws);
            let Some((i, term)) = vague(&ws) else {
                continue;
            };
            if !is_directive(&ws) || anchored(text, &ws) {
                continue;
            }
            let start = r.start + offset(text, ws[i].text);
            hits.push(Hit {
                span: span(start, start + ws[i].text.len()),
                quote: term,
                token_waste: 0,
            });
        }
        aggregate(
            &META_VAGUE,
            asset,
            &hits,
            |h| format!("Vague directive \"{}\": say what it means here", h.quote),
            |n, top| format!("{n} vague directives: {top}"),
            out,
        );
    }
}

/// A lexicon phrase split into lemmatized words, once (not per sentence word).
type Phrase = (&'static str, Vec<&'static str>);

fn lemmatize(list: &'static [&'static str]) -> Vec<Phrase> {
    list.iter()
        .map(|&p| (p, p.split_whitespace().map(lemma).collect()))
        .filter(|(_, ws): &Phrase| !ws.is_empty())
        .collect()
}

static VAGUE_PHRASES: LazyLock<Vec<Phrase>> = LazyLock::new(|| lemmatize(VAGUE));
static ANCHOR_PHRASES: LazyLock<Vec<Phrase>> = LazyLock::new(|| lemmatize(ANCHORS));

/// Index of the first word of the first [`VAGUE`] term in `w`, with the term.
fn vague(w: &[Word<'_>]) -> Option<(usize, &'static str)> {
    find_phrase(w, &VAGUE_PHRASES)
}

/// The first position in `w` where one of `phrases` occurs (lemmas compared), in word
/// order then lexicon order.
fn find_phrase(w: &[Word<'_>], phrases: &[Phrase]) -> Option<(usize, &'static str)> {
    (0..w.len()).find_map(|i| {
        let first = lemma(w[i].text);
        phrases
            .iter()
            .find(|(_, p)| {
                p.first().is_some_and(|f| first.eq_ignore_ascii_case(f))
                    && w.len() - i >= p.len()
                    && p[1..]
                        .iter()
                        .zip(&w[i + 1..])
                        .all(|(p, x)| lemma(x.text).eq_ignore_ascii_case(p))
            })
            .map(|&(t, _)| (i, t))
    })
}

/// AI-COG-002 anchor: a digit, a code span, a path or file name, or an example.
fn anchored(text: &str, w: &[Word<'_>]) -> bool {
    text.bytes()
        .any(|b| b.is_ascii_digit() || b == b'`' || b == b'/')
        || w.iter().any(|x| x.text.contains('.'))
        || find_phrase(w, &ANCHOR_PHRASES).is_some()
}

pub static META_NEGATIVE: RuleMeta = RuleMeta {
    id: "AI-COG-003",
    name: "Negative-only constraint",
    category: Category::Cognitive,
    default_severity: Severity::Info,
    kinds: KindSet::MARKDOWN,
    summary: "A directive says what not to do without saying what to do instead.",
    remediation: "State what to do instead. Models follow positive instructions more reliably.",
    inline_suppressible: true,
};

/// Words before an [`ALTERNATIVES`] cue searched for a negator (`do not ever use`).
const NEGATOR_WINDOW: usize = 2;

pub struct NegativeOnly;

impl Rule for NegativeOnly {
    fn meta(&self) -> &'static RuleMeta {
        &META_NEGATIVE
    }

    fn check(&self, asset: &Asset, _ctx: &RuleCtx<'_>, out: &mut Vec<Finding>) {
        let src = asset.source.as_str();
        let negatives: Vec<_> = extract(asset)
            .into_iter()
            .filter(|d| d.polarity == Polarity::Negative)
            .collect();
        if negatives.is_empty() {
            return;
        }
        let sents: Vec<_> = asset.prose().sentences().collect();
        let mut ws = Vec::new();
        let mut hits = Vec::new();
        for d in &negatives {
            // AI-COG-003: an alternative in the same or the next sentence.
            let i = sents.partition_point(|s| s.range.start < d.span.start as usize);
            let offers = sents.iter().skip(i).take(2).enumerate().any(|(k, s)| {
                let text = src.get(s.range.clone()).unwrap_or_default();
                s.words(src, &mut ws);
                alternative(&ws) || k == 0 && positive_clause(text, &ws)
            });
            if !offers {
                hits.push(Hit {
                    span: d.span,
                    quote: src
                        .get(d.span.start as usize..d.span.end as usize)
                        .unwrap_or_default(),
                    token_waste: 0,
                });
            }
        }
        aggregate(
            &META_NEGATIVE,
            asset,
            &hits,
            |h| {
                let q = truncate(h.quote, 40);
                format!("Negative-only constraint \"{q}\": say what to do instead")
            },
            |n, top| format!("{n} negative-only constraints: {top}"),
            out,
        );
    }
}

/// Whether `w` offers a positive alternative: an [`ALTERNATIVES`] word with no negator in
/// the [`NEGATOR_WINDOW`] words before it.
fn alternative(w: &[Word<'_>]) -> bool {
    w.iter().enumerate().any(|(i, x)| {
        is(x.text, ALTERNATIVES)
            && !w[i.saturating_sub(NEGATOR_WINDOW)..i]
                .iter()
                .any(|p| is(p.text, NEGATORS))
    })
}

/// AI-COG-003 FP guards within the directive's own sentence: a contrast that already states
/// the positive (`A bad asset becomes a finding, never a panic`), or a later clause opening
/// with an imperative (`Never put secrets in messages: redact them`).
fn positive_clause(text: &str, w: &[Word<'_>]) -> bool {
    let Some(n) = w
        .iter()
        .enumerate()
        .position(|(i, x)| is(x.text, NEGATORS) || i == 0 && is(x.text, NEGATIVE_FIRST))
    else {
        return false;
    };
    let before = text.get(..offset(text, w[n].text)).unwrap_or_default();
    let contrast =
        n > 0 && before.trim_end().ends_with([',', '—', '–']) && !is(w[0].text, QUALIFIERS);
    contrast
        || w.iter().enumerate().skip(n + 1).any(|(j, x)| {
            x.brk
                && !is(x.text, NEGATORS)
                && (is(x.text, IMPERATIVES) || is(x.text, DIRECTIVE_VERBS))
                && !w.get(j + 1).is_some_and(|y| is(y.text, NEGATORS))
        })
}

pub static META_EMPHASIS: RuleMeta = RuleMeta {
    id: "AI-COG-004",
    name: "Emphasis inflation",
    category: Category::Cognitive,
    default_severity: Severity::Low,
    kinds: KindSet::MARKDOWN,
    summary: "Too many directives are emphasized, so emphasis no longer stands out.",
    remediation: "Keep caps, bold and \"!!\" for the one or two rules that truly matter.",
    inline_suppressible: true,
};

/// Emphasized directives needed before the share is checked.
const MIN_EMPHASIZED: usize = 4;

pub struct EmphasisInflation;

impl Rule for EmphasisInflation {
    fn meta(&self) -> &'static RuleMeta {
        &META_EMPHASIS
    }

    fn check(&self, asset: &Asset, _ctx: &RuleCtx<'_>, out: &mut Vec<Finding>) {
        let src = asset.source.as_str();
        let mut ws = Vec::new();
        let (mut directives, mut emphasized) = (0usize, 0usize);
        let mut first = None;
        for s in asset.prose().sentences() {
            let Some(text) = src.get(s.range.clone()) else {
                continue;
            };
            s.words(src, &mut ws);
            if !is_directive(&ws) {
                continue;
            }
            directives += 1;
            if emphasis(text, &ws) {
                emphasized += 1;
                first.get_or_insert(span(s.range.start, s.range.end));
            }
        }
        // AI-COG-004: > 25% of directives and at least 4 of them.
        if emphasized < MIN_EMPHASIZED || emphasized * 4 <= directives {
            return;
        }
        let Some(at) = first else {
            return;
        };
        let pct = emphasized * 100 / directives;
        let msg = format!(
            "{emphasized} of {directives} directives ({pct}%) are emphasized; none stands out"
        );
        out.push(Finding::new(&META_EMPHASIS, asset, at, msg));
    }
}

/// Whether a directive sentence carries an AI-COG-004 emphasis marker.
fn emphasis(text: &str, w: &[Word<'_>]) -> bool {
    text.bytes().filter(|&b| b == b'!').count() >= 2
        || fully_bold(text)
        || w.iter().any(|x| {
            let t = x.text;
            let caps = t.len() >= 4 && t.bytes().all(|b| b.is_ascii_uppercase());
            (EMPHASIS.contains(&t) || caps && !ACRONYMS.contains(&t))
                && !in_code(text, offset(text, t))
        })
}

/// `**Whole sentence.**` (or `__…__`), not `**a** and **b**`.
fn fully_bold(text: &str) -> bool {
    let t = text.trim_end_matches(['.', '!', '?', ':']);
    ["**", "__"].iter().any(|d| {
        t.len() > 2 * d.len()
            && t.starts_with(d)
            && t.ends_with(d)
            && t.get(d.len()..t.len() - d.len())
                .is_some_and(|inner| !inner.contains(d))
    })
}

/// Whether byte `off` of `text` is inside an inline code span.
fn in_code(text: &str, off: usize) -> bool {
    text.get(..off)
        .is_some_and(|b| b.bytes().filter(|&c| c == b'`').count() % 2 == 1)
}

/// Byte offset of `word` (a slice of `text`) in `text`.
fn offset(text: &str, word: &str) -> usize {
    (word.as_ptr() as usize).saturating_sub(text.as_ptr() as usize)
}

fn span(start: usize, end: usize) -> Span {
    let c = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
    Span::new(c(start), c(end))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cognitive::directives::words;

    fn guarded(s: &str) -> bool {
        let mut w = Vec::new();
        words(s, &mut w);
        positive_clause(s, &w)
    }

    #[test]
    fn positive_clause_guards() {
        assert!(guarded("A bad asset becomes a finding, never a panic."));
        assert!(guarded("Never put secrets in messages: redact them."));
        assert!(!guarded("In tests, never hit the network."));
        assert!(!guarded("Never push to main, avoid force pushes."));
        assert!(!guarded("IMPORTANT: never push to main."));
    }
}
