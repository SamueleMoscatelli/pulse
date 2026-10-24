//! Directive extraction shared by AI-COG-001 and AI-COG-005 (spec AI-COG-001 → Detect 1).
//!
//! The Markdown body (outside code blocks, HTML comments and headings) is split into list
//! items and paragraphs, then into sentences. A sentence with a polarity marker becomes a
//! [`Directive`]. Words are source slices: compare them with [`same`] (ASCII case-insensitive).

use pulse_core::text::prose::{Prose, Sentence};
pub use pulse_core::text::prose::{Word, covers, sentences, split, words};
use pulse_core::{Asset, MarkdownView, Span};

use crate::lexicon::directives::{
    MODALS, NEGATIVE, NEGATIVE_FIRST, NEGATORS, POSITIVE, POSITIVE_FIRST, QUALIFIERS, STOP,
    TERMINATORS, USE_VERBS,
};
use crate::lexicon::imperatives::IMPERATIVES;
use crate::lexicon::style::DIRECTIVE_VERBS;

/// Most content words kept in an object or a context.
const MAX_WORDS: usize = 4;

/// Whether a directive prescribes or forbids.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Polarity {
    Positive,
    Negative,
}

/// One prescriptive sentence or list item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Directive<'a> {
    pub polarity: Polarity,
    /// Main verb; `"use"` for use/prefer/avoid and verb-less markers (`No semicolons`).
    pub verb: &'a str,
    /// 1–4 content-word lemmas after the verb.
    pub object: Vec<&'a str>,
    /// Content-word lemmas after the first scoping qualifier (`for tests` → `["test"]`).
    pub context: Vec<&'a str>,
    /// The whole sentence, trimmed.
    pub span: Span,
    /// Text of the nearest heading above, if any.
    pub section: Option<&'a str>,
    /// That heading is the document title: the file's first heading, at level 1.
    pub titled: bool,
}

/// Every directive in `asset`'s Markdown body, in source order.
pub fn extract(asset: &Asset) -> Vec<Directive<'_>> {
    let mut x = Extractor {
        src: &asset.source,
        md: asset.markdown(),
        prose: asset.prose(),
        words: Vec::new(),
        out: Vec::new(),
    };
    x.run();
    x.out
}

/// ASCII case-insensitive word equality.
pub fn same(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// Whether `w` is one of `list`.
pub fn is(w: &str, list: &[&str]) -> bool {
    list.iter().any(|l| same(w, l))
}

/// Naive lemma: drops a possessive `'s` and a plural `s` (not `ss`) from words over 3 bytes.
pub fn lemma(w: &str) -> &str {
    let w = w
        .strip_suffix("'s")
        .or_else(|| w.strip_suffix("’s"))
        .unwrap_or(w);
    let b = w.as_bytes();
    match b {
        [.., p, b's' | b'S'] if b.len() > 3 && !matches!(p, b's' | b'S') => &w[..w.len() - 1],
        _ => w,
    }
}

/// Whether the `phrase` (whitespace-separated words, lemmatized here) occurs contiguously
/// in `words`.
pub fn contains_phrase(words: &[&str], phrase: &str) -> bool {
    let mut parts = phrase.split_whitespace();
    let Some(first) = parts.next().map(lemma) else {
        return false;
    };
    // Anchored on the first word, so the rest is split and lemmatized only where it
    // matches (once per window was a hotspot, P7.1).
    (0..words.len()).any(|i| {
        same(words[i], first) && {
            let mut rest = words[i + 1..].iter();
            parts
                .clone()
                .all(|p| rest.next().is_some_and(|w| same(w, lemma(p))))
        }
    })
}

/// [`contains_phrase`] with the phrase already split and lemmatized.
pub fn contains_words(words: &[&str], phrase: &[&str]) -> bool {
    let Some((first, rest)) = phrase.split_first() else {
        return false;
    };
    (0..words.len()).any(|i| {
        same(words[i], first) && {
            let mut tail = words[i + 1..].iter();
            rest.iter().all(|p| tail.next().is_some_and(|w| same(w, p)))
        }
    })
}

/// Whether a sentence's words make it a directive for the style rules (AI-COG-002,
/// AI-COG-004): an AI-COG-001 marker anywhere, or an imperative verb opening a clause
/// (`Write …`, `IMPORTANT: run …`).
pub fn is_directive(w: &[Word<'_>]) -> bool {
    w.iter().enumerate().any(|(i, x)| {
        let t = x.text;
        is(t, POSITIVE)
            || is(t, NEGATIVE)
            || (i == 0 || x.brk)
                && (is(t, POSITIVE_FIRST)
                    || is(t, NEGATIVE_FIRST)
                    || is(t, IMPERATIVES)
                    || is(t, DIRECTIVE_VERBS))
    })
}

struct Extractor<'a> {
    src: &'a str,
    md: &'a MarkdownView,
    prose: &'a Prose,
    /// Reused word buffer: no allocation per non-directive sentence.
    words: Vec<Word<'a>>,
    out: Vec<Directive<'a>>,
}

impl<'a> Extractor<'a> {
    fn run(&mut self) {
        for s in self.prose.sentences() {
            self.sentence(&s);
        }
    }

    fn sentence(&mut self, s: &Sentence<'_>) {
        let src = self.src;
        let Some(text) = src.get(s.range.clone()) else {
            return;
        };
        let start = s.range.start;
        let mut ws = std::mem::take(&mut self.words);
        s.words(src, &mut ws);
        if let Some((polarity, verb, object, context)) = parse(&ws) {
            let md = self.md;
            let n = md
                .headings
                .partition_point(|h| h.span.start as usize <= start);
            let section = md.headings[..n].last().map(|h| h.text.as_str());
            let titled = n == 1 && md.headings[0].level == 1;
            self.out.push(Directive {
                polarity,
                verb,
                object,
                context,
                span: span(start, start + text.len()),
                section,
                titled,
            });
        }
        self.words = ws;
    }
}

type Parsed<'a> = (Polarity, &'a str, Vec<&'a str>, Vec<&'a str>);

/// Polarity, verb, object and context of a sentence's words, if it is a directive.
fn parse<'a>(w: &[Word<'a>]) -> Option<Parsed<'a>> {
    let opens = |i: usize, x: &str| {
        is(x, POSITIVE)
            || is(x, NEGATIVE)
            || i == 0 && (is(x, POSITIVE_FIRST) || is(x, NEGATIVE_FIRST))
            // `We do not use yarn`: a negated `do` opens anywhere.
            || same(x, "do") && w.get(i + 1).is_some_and(|n| is(n.text, NEGATORS))
    };
    let first = w.iter().enumerate().position(|(i, x)| opens(i, x.text))?;
    // Marker run: `must not`, `should always use`, `avoid using`, `do not`.
    let (mut j, mut negative, mut verb) = (first, false, None);
    while let Some(x) = w.get(j) {
        let t = x.text;
        if j > first && x.brk {
            break;
        }
        if is(t, NEGATORS) || j == 0 && is(t, NEGATIVE_FIRST) {
            negative = true;
            if same(t, "avoid") {
                verb = Some("use");
            }
        } else if is(t, USE_VERBS) {
            verb = Some("use");
        } else if !is(t, MODALS) {
            break;
        }
        j += 1;
    }
    let verb = match verb {
        Some(v) => v,
        // `No semicolons`: the object follows the marker directly.
        None if first == 0 && j == 1 && is(w[0].text, NEGATIVE_FIRST) => "use",
        None => {
            let v = w.get(j).filter(|x| !x.brk)?.text;
            j += 1;
            v
        }
    };
    let mut object = Vec::new();
    let mut k = j;
    while let Some(x) = w.get(k) {
        let t = x.text;
        let leading_only = k == j && same(t, "only"); // `use only TypeScript`
        if k > j && x.brk
            || object.len() == MAX_WORDS
            || !leading_only && is(t, QUALIFIERS)
            || is(t, TERMINATORS)
        {
            break;
        }
        if !leading_only && !is(t, STOP) {
            object.push(lemma(t));
        }
        k += 1;
    }
    if object.is_empty() {
        return None;
    }
    // AI-COG-001 FP guard input: the words after the first qualifier outside verb + object.
    let q = (0..first)
        .chain(k..w.len())
        .find(|&i| is(w[i].text, QUALIFIERS));
    let mut context = Vec::new();
    if let Some(q) = q {
        let limit = if q < first { first } else { w.len() };
        for x in &w[q + 1..limit] {
            if x.brk || context.len() == MAX_WORDS || is(x.text, QUALIFIERS) {
                break;
            }
            if !is(x.text, STOP) {
                context.push(lemma(x.text));
            }
        }
    }
    let polarity = if negative {
        Polarity::Negative
    } else {
        Polarity::Positive
    };
    Some((polarity, verb, object, context))
}

fn span(start: usize, end: usize) -> Span {
    let c = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
    Span::new(c(start), c(end))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pulse_core::AssetKind;
    use std::path::PathBuf;

    fn ds(src: &str) -> Vec<(Polarity, String, String, String)> {
        let a = Asset::from_bytes(
            PathBuf::from("CLAUDE.md"),
            "CLAUDE.md",
            AssetKind::Instructions,
            AssetKind::Instructions.default_load(),
            src.as_bytes().to_vec(),
        );
        extract(&a)
            .into_iter()
            .map(|d| {
                (
                    d.polarity,
                    d.verb.to_lowercase(),
                    d.object.join(" ").to_lowercase(),
                    d.context.join(" ").to_lowercase(),
                )
            })
            .collect()
    }

    fn one(src: &str) -> (Polarity, String, String, String) {
        let v = ds(src);
        assert_eq!(v.len(), 1, "{src:?} → {v:?}");
        v.into_iter().next().unwrap_or_else(|| unreachable!())
    }

    #[test]
    fn polarity_verb_and_object() {
        use Polarity::*;
        let s = |p, v: &str, o: &str, c: &str| (p, v.into(), o.into(), c.into());
        assert_eq!(
            one("Always use semicolons."),
            s(Positive, "use", "semicolon", "")
        );
        assert_eq!(
            one("Never use semicolons"),
            s(Negative, "use", "semicolon", "")
        );
        assert_eq!(
            one("Do not commit the secrets."),
            s(Negative, "commit", "secret", "")
        );
        assert_eq!(
            one("Avoid using `any` types"),
            s(Negative, "use", "any type", "")
        );
        assert_eq!(one("No semicolons."), s(Negative, "use", "semicolon", ""));
        assert_eq!(
            one("Prefer double quotes over single quotes"),
            s(Positive, "use", "double quote", "")
        );
        assert_eq!(
            one("Use npm for scripts, pnpm for the workspace"),
            s(Positive, "use", "npm", "script")
        );
        assert_eq!(
            one("In Python, always use spaces."),
            s(Positive, "use", "space", "python")
        );
        assert_eq!(
            one("Use only TypeScript"),
            s(Positive, "use", "typescript", "")
        );
        for neg in [
            "We do not use yarn.",
            "Please do not use yarn",
            "You should not use yarn",
        ] {
            assert_eq!(one(neg), s(Negative, "use", "yarn", ""), "{neg}");
        }
        assert_eq!(
            one("Tests must not hit the network."),
            s(Negative, "hit", "network", "")
        );
    }

    #[test]
    fn non_directives_and_code_are_skipped() {
        assert!(ds("This project is a CLI.\nNo.\n").is_empty());
        assert!(ds("```md\nAlways use tabs.\n```\n<!-- never use tabs -->\n").is_empty());
        assert!(ds("# Always use tabs\n").is_empty());
        assert!(ds("what to do with it").is_empty());
    }

    #[test]
    fn splits_list_items_sentences_and_keeps_inline_code() {
        let v =
            ds("- Always use tabs. Never use `foo.bar`; prefer x\n- No semicolons\n  in JSON\n");
        let objs: Vec<_> = v.iter().map(|d| d.2.as_str()).collect();
        assert_eq!(objs, ["tab", "foo.bar", "x", "semicolon"]);
        assert_eq!(v[3].3, "json");
    }

    #[test]
    fn spans_and_sections() {
        let src = "# Rules\n## Python\n- Always use spaces.\n";
        let a = Asset::from_bytes(
            PathBuf::from("CLAUDE.md"),
            "CLAUDE.md",
            AssetKind::Instructions,
            AssetKind::Instructions.default_load(),
            src.as_bytes().to_vec(),
        );
        let d = extract(&a);
        assert_eq!(d.len(), 1);
        assert_eq!(
            &src[d[0].span.start as usize..d[0].span.end as usize],
            "Always use spaces."
        );
        assert_eq!(d[0].section, Some("Python"));
    }

    #[test]
    fn hostile_input_does_not_panic() {
        let v = ds("- Always use `tabs.\r\n- Nie używaj spacji. Never use ünïcode\r\n");
        assert_eq!(v.len(), 2, "{v:?}");
        assert_eq!(v[1].2, "ünïcode");
        assert!(ds("`").is_empty());
        assert!(ds("- \n1.\n").is_empty());
    }

    #[test]
    fn lemma_and_phrase() {
        assert_eq!(lemma("tabs"), "tab");
        assert_eq!(lemma("class"), "class");
        assert_eq!(lemma("js"), "js");
        assert_eq!(lemma("user's"), "user");
        assert!(contains_phrase(
            &["prefer", "single", "quote"],
            "single quotes"
        ));
        assert!(!contains_phrase(&["single"], "single quote"));
        assert!(!contains_phrase(&["x"], ""));
        assert!(contains_words(
            &["prefer", "Single", "quote"],
            &["single", "quote"]
        ));
        assert!(!contains_words(&["single"], &["single", "quote"]));
        assert!(!contains_words(&["x"], &[]));
    }

    #[test]
    fn style_directives() {
        let d = |s: &str| {
            let mut w = Vec::new();
            words(s, &mut w);
            is_directive(&w)
        };
        assert!(d("Write clean code."));
        assert!(d("IMPORTANT: run the tests"));
        assert!(d("You must keep it short"));
        assert!(!d("This project is a CLI."));
        assert!(!d("The test suite is slow"));
    }
}
