//! Directive style: vague directives (AI-COG-002), negative-only constraints (AI-COG-003)
//! and emphasis inflation (AI-COG-004).

use std::sync::LazyLock;

use pulse_core::{Asset, Category, Finding, KindSet, Rule, RuleCtx, RuleMeta, Severity, Span};

use super::directives::{Word, is, is_directive, lemma, same};
use crate::lexicon::directives::{
    ALTERNATIVES, LEAD_INS, NEGATIVE, NEGATORS, PROHIBITING_MODALS, QUALIFIERS, SUBJECTS,
    TERMINATORS, VAGUE_LIMITS,
};
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
    summary: "A vague prohibition says what not to do without saying what to do instead.",
    remediation: "State what to do instead. Models follow positive instructions more reliably.",
    inline_suppressible: true,
};

/// Words before an [`ALTERNATIVES`] cue searched for a negator (`do not ever use`).
const NEGATOR_WINDOW: usize = 2;

/// Prohibitions needed before their share of the directives is checked.
const MIN_PROHIBITIONS: usize = 8;

static VAGUE_LIMIT_PHRASES: LazyLock<Vec<Phrase>> = LazyLock::new(|| {
    let mut v = lemmatize(VAGUE);
    v.extend(lemmatize(VAGUE_LIMITS));
    v
});

pub struct NegativeOnly;

impl Rule for NegativeOnly {
    fn meta(&self) -> &'static RuleMeta {
        &META_NEGATIVE
    }

    fn check(&self, asset: &Asset, _ctx: &RuleCtx<'_>, out: &mut Vec<Finding>) {
        let src = asset.source.as_str();
        let sents: Vec<_> = asset.prose().sentences().collect();
        let mut ws = Vec::new();
        let mut next = Vec::new();
        let mut hits = Vec::new();
        let (mut directives, mut prohibitions) = (0usize, 0usize);
        let mut first = None;
        for (i, s) in sents.iter().enumerate() {
            let Some(text) = src.get(s.range.clone()) else {
                continue;
            };
            s.words(src, &mut ws);
            let Some(n) = prohibition(text, &ws) else {
                directives += usize::from(is_directive(&ws));
                continue;
            };
            directives += 1;
            prohibitions += 1;
            let sp = span(s.range.start, s.range.end);
            first.get_or_insert(sp);
            // AI-COG-003 (1): a vague, non-concrete prohibition with no alternative in the
            // same or the next sentence.
            let rest = text.get(offset(text, ws[n].text)..).unwrap_or_default();
            // The object ends at a scope or reason (`… when X is more appropriate`).
            let end = (n + 1..ws.len())
                .find(|&k| is(ws[k].text, QUALIFIERS) || is(ws[k].text, TERMINATORS))
                .unwrap_or(ws.len());
            if concrete(rest) || find_phrase(&ws[n..end], &VAGUE_LIMIT_PHRASES).is_none() {
                continue;
            }
            let offers = alternative(&ws)
                || positive_clause(text, &ws, n)
                || sents.get(i + 1).is_some_and(|s| {
                    s.words(src, &mut next);
                    alternative(&next)
                });
            if !offers {
                hits.push(Hit {
                    span: sp,
                    quote: text,
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
                format!("Vague prohibition \"{q}\": say what to do instead")
            },
            |n, top| format!("{n} vague prohibitions: {top}"),
            out,
        );
        // AI-COG-003 (2): prohibitions are >= 60% of the directives and at least 8.
        if let Some(at) = first
            && prohibitions >= MIN_PROHIBITIONS
            && prohibitions * 5 >= directives * 3
        {
            let msg = format!(
                "Mostly prohibitions ({prohibitions} of {directives} directives): \
                 lead with what good output looks like"
            );
            let mut f = Finding::new(&META_NEGATIVE, asset, at, msg);
            f.severity = Severity::Low;
            out.push(f);
        }
    }
}

/// Index in `w` of the negator of a prohibition opening a clause of `text` (the sentence
/// start, or after a `:` label): `Do not`, `Don't`, `Never`, `Avoid`, `You must not`, after
/// optional [`LEAD_INS`]. Not in code spans or quotes.
fn prohibition(text: &str, w: &[Word<'_>]) -> Option<usize> {
    (0..w.len()).find_map(|i| {
        if i > 0 && !(w[i].brk && after_label(text, w[i].text)) {
            return None;
        }
        let mut j = i;
        while w.get(j).is_some_and(|x| is(x.text, LEAD_INS)) && j - i < 2 {
            j += 1;
        }
        let subject = w.get(j).is_some_and(|x| is(x.text, SUBJECTS));
        j += usize::from(subject);
        let x = w.get(j)?;
        let next_negates = || w.get(j + 1).is_some_and(|y| is(y.text, NEGATORS));
        let n = if is(x.text, PROHIBITING_MODALS) && next_negates() {
            j + 1
        } else if subject {
            // `You mustn't`; `You don't need …` and `You never …` state facts.
            (is(x.text, NEGATORS) && x.text.len() > 5 && x.text.contains(['\'', '’']))
                .then_some(j)?
        } else if is(x.text, NEGATIVE) || same(x.text, "do") && next_negates() {
            j + usize::from(same(x.text, "do"))
        } else {
            return None;
        };
        let off = offset(text, w[n].text);
        (!in_code(text, off) && !quoted(text, off)).then_some(n)
    })
}

/// Whether `word` (a slice of `text`) follows a `:` label (`Note: never …`, `**Rule**: …`).
fn after_label(text: &str, word: &str) -> bool {
    text.get(..offset(text, word))
        .is_some_and(|b| b.trim_end().trim_end_matches(['*', '_']).ends_with(':'))
}

/// Whether byte `off` of `text` is inside a double-quoted string.
fn quoted(text: &str, off: usize) -> bool {
    text.get(..off).is_some_and(|b| {
        b.bytes().filter(|&c| c == b'"').count() % 2 == 1
            || b.matches('“').count() > b.matches('”').count()
    })
}

/// AI-COG-003: a prohibition whose object is concrete: a code span, a quoted literal, a
/// path, a number or a file name.
fn concrete(rest: &str) -> bool {
    rest.bytes()
        .any(|b| matches!(b, b'`' | b'"' | b'/' | b'\\') || b.is_ascii_digit())
        || rest.contains('“')
        || rest.split_whitespace().any(|t| {
            let t = t.trim_matches(|c: char| !c.is_alphanumeric());
            t.split_once('.')
                .is_some_and(|(a, b)| a.len() > 1 && b.len() > 1)
        })
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

/// AI-COG-003 FP guards within the prohibition's own sentence (negator at `w[n]`): a
/// contrast that already states the positive (`Keep it short, never a wall of text`), or a
/// later clause opening with an imperative (`Never put secrets in messages: redact them`,
/// `Avoid jargon and keep it short`).
fn positive_clause(text: &str, w: &[Word<'_>], n: usize) -> bool {
    let before = text.get(..offset(text, w[n].text)).unwrap_or_default();
    let contrast =
        n > 0 && before.trim_end().ends_with([',', '—', '–']) && !is(w[0].text, QUALIFIERS);
    contrast
        || w.iter().enumerate().skip(n + 1).any(|(j, x)| {
            (x.brk || j > n + 1 && is(w[j - 1].text, &["and", "but"]))
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
    remediation: "Keep emphasis for the rules that must never be missed; state the others plainly.",
    inline_suppressible: true,
};

/// Emphasized directives needed before the share is checked.
const MIN_EMPHASIZED: usize = 4;

/// Lines of emphasized directives listed in the message.
const LISTED_LINES: usize = 3;

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
        let mut lines: Vec<u32> = Vec::new();
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
                let at = span(s.range.start, s.range.end);
                first.get_or_insert(at);
                let line = asset.lines.line_of(at.start);
                if lines.len() <= LISTED_LINES && lines.last() != Some(&line) {
                    lines.push(line);
                }
            }
        }
        // AI-COG-004: > 25% of directives and at least 4 of them.
        if emphasized < MIN_EMPHASIZED || emphasized * 4 <= directives {
            return;
        }
        let Some(at) = first else {
            return;
        };
        let mut listed: Vec<String> = lines
            .iter()
            .take(LISTED_LINES)
            .map(u32::to_string)
            .collect();
        if lines.len() > LISTED_LINES {
            listed.push("…".into());
        }
        let msg = format!(
            "Emphasis on {emphasized} of {directives} directives (lines {}); \
             emphasis works only when rare (max 25%)",
            listed.join(", ")
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
        let n = w.iter().position(|x| is(x.text, NEGATORS)).unwrap_or(0);
        positive_clause(s, &w, n)
    }

    fn prohibits(s: &str) -> bool {
        let mut w = Vec::new();
        words(s, &mut w);
        prohibition(s, &w).is_some()
    }

    #[test]
    fn prohibitions_open_a_clause() {
        for s in [
            "Never push to main.",
            "Do not edit the lockfile.",
            "Don't mock the database.",
            "Avoid global state.",
            "You must not log tokens.",
            "We should never retry twice.",
            "Respect privacy: do not include names.",
            "**Completeness**: Don't omit resources.",
            "Please never guess.",
            "otherwise do not attempt to shuffle.",
            "You mustn't skip tests.",
        ] {
            assert!(prohibits(s), "{s}");
        }
        for s in [
            "They do not have a manifest.",
            "To avoid conflicts, run it first.",
            "No build step.",
            "No Authentication",
            "If you don't know, ask.",
            "You do not need to restart.",
            "A bad asset becomes a finding, never a panic.",
            "Say \"Never mind\" to cancel.",
            "Run `never-fail` first.",
            "The user should not see this.",
        ] {
            assert!(!prohibits(s), "{s}");
        }
    }

    #[test]
    fn concrete_objects() {
        assert!(concrete("Do not add `x-awesome` fields"));
        assert!(concrete("Never commit .azure/ folders"));
        assert!(concrete("Never edit package.json by hand"));
        assert!(concrete("Do not exceed 80 columns"));
        assert!(concrete("Never write \"TODO\" comments"));
        assert!(!concrete("Do not add unnecessary comments."));
        assert!(!concrete("Avoid bad names, e.g. foo."));
    }

    #[test]
    fn positive_clause_guards() {
        assert!(guarded("Keep it short, never a wall of text."));
        assert!(guarded("Never put secrets in messages: redact them."));
        assert!(guarded("Avoid unnecessary complexity and keep it short."));
        assert!(!guarded("In tests, never hit the network."));
        assert!(!guarded("Never push to main, avoid force pushes."));
        assert!(!guarded("IMPORTANT: never push to main."));
    }
}
