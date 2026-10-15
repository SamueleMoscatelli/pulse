//! Directive extraction shared by AI-COG-001 and AI-COG-005 (spec AI-COG-001 → Detect 1).
//!
//! The Markdown body (outside code blocks, HTML comments and headings) is split into list
//! items and paragraphs, then into sentences. A sentence with a polarity marker becomes a
//! [`Directive`]. Words are source slices: compare them with [`same`] (ASCII case-insensitive).

use std::ops::Range;

use memchr::memchr;
use pulse_core::{Asset, MarkdownView, Span};

use crate::lexicon::directives::{
    MODALS, NEGATIVE, NEGATIVE_FIRST, NEGATORS, POSITIVE, POSITIVE_FIRST, QUALIFIERS, STOP,
    TERMINATORS, USE_VERBS,
};

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
}

/// Every directive in `asset`'s Markdown body, in source order.
pub fn extract(asset: &Asset) -> Vec<Directive<'_>> {
    let mut x = Extractor {
        src: &asset.source,
        md: asset.markdown(),
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
    let n = phrase.split_whitespace().count();
    n > 0
        && words.windows(n).any(|win| {
            win.iter()
                .zip(phrase.split_whitespace())
                .all(|(w, p)| same(w, lemma(p)))
        })
}

/// A word of a sentence; `brk` marks punctuation (`,` `:` `(` …) right before it.
#[derive(Debug, Clone, Copy)]
pub struct Word<'a> {
    pub text: &'a str,
    pub brk: bool,
}

/// Splits `s` into words into `out` (cleared first). Word chars are alphanumerics and
/// `_+#`; `-` `.` `'` `’` join two word chars (`kebab-case`, `node.js`, `don't`).
pub fn words<'a>(s: &'a str, out: &mut Vec<Word<'a>>) {
    out.clear();
    let mut brk = false;
    let mut start = None;
    let mut it = s.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        let joins = matches!(c, '-' | '.' | '\'' | '’')
            && start.is_some()
            && it.peek().is_some_and(|&(_, n)| n.is_alphanumeric());
        if c.is_alphanumeric() || matches!(c, '_' | '+' | '#') || joins {
            start.get_or_insert(i);
            continue;
        }
        if let Some(st) = start.take() {
            out.push(Word {
                text: &s[st..i],
                brk,
            });
            brk = false;
        }
        if matches!(c, ',' | ':' | '(' | ')' | '—' | '–') {
            brk = true;
        }
    }
    if let Some(st) = start {
        out.push(Word {
            text: &s[st..],
            brk,
        });
    }
}

/// Calls `f` with the trimmed, non-empty source range of every sentence in `md`'s body,
/// outside code blocks, HTML comments and headings, in source order. List items and
/// paragraphs are split on `.` `!` `?` (before whitespace or the end), `;` and `|`, outside
/// inline code.
pub fn sentences(src: &str, md: &MarkdownView, mut f: impl FnMut(Range<usize>)) {
    let (mut pos, end) = (md.body.start as usize, md.body.end as usize);
    let bytes = src.as_bytes();
    let mut unit: Option<Range<usize>> = None;
    let flush = |unit: Option<Range<usize>>, f: &mut dyn FnMut(Range<usize>)| {
        if let Some(u) = unit {
            split(src, u, f);
        }
    };
    while pos < end {
        let eol = bytes
            .get(pos..end)
            .and_then(|b| memchr(b'\n', b))
            .map_or(end, |n| pos + n);
        let line = src.get(pos..eol).unwrap_or("");
        let trimmed = line.trim_start();
        let at = pos + (line.len() - trimmed.len());
        if trimmed.trim_end().is_empty()
            || covers(&md.code_blocks, at, |b| b.span)
            || covers(&md.html_comments, at, |s| *s)
            || covers(&md.headings, at, |h| h.span)
        {
            flush(unit.take(), &mut f);
        } else if let Some(marker) = list_marker(trimmed) {
            flush(unit.take(), &mut f);
            unit = Some(at + marker..eol);
        } else {
            unit = Some(unit.map_or(at, |u| u.start)..eol);
        }
        pos = eol + 1;
    }
    flush(unit, &mut f);
}

/// Calls `f` with the trimmed, non-empty range of every sentence of `src[unit]` (a paragraph
/// or list item, or any plain text such as a tool description).
pub fn split(src: &str, unit: Range<usize>, f: &mut dyn FnMut(Range<usize>)) {
    let Some(text) = src.get(unit.clone()) else {
        return;
    };
    let mut emit = |r: Range<usize>| {
        let Some(raw) = src.get(r.clone()) else {
            return;
        };
        let t = raw.trim();
        if !t.is_empty() {
            let start = r.start + (raw.len() - raw.trim_start().len());
            f(start..start + t.len());
        }
    };
    let mut start = 0;
    let mut code = false;
    let mut it = text.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        match c {
            '`' => code = !code,
            '.' | '!' | '?' if !code && it.peek().is_none_or(|&(_, n)| n.is_whitespace()) => {
                emit(unit.start + start..unit.start + i + 1);
                start = i + 1;
            }
            ';' | '|' if !code => {
                emit(unit.start + start..unit.start + i);
                start = i + 1;
            }
            _ => {}
        }
    }
    emit(unit.start + start..unit.end);
}

struct Extractor<'a> {
    src: &'a str,
    md: &'a MarkdownView,
    /// Reused word buffer: no allocation per non-directive sentence.
    words: Vec<Word<'a>>,
    out: Vec<Directive<'a>>,
}

impl<'a> Extractor<'a> {
    fn run(&mut self) {
        sentences(self.src, self.md, |r| self.sentence(r));
    }

    fn sentence(&mut self, range: Range<usize>) {
        let src = self.src;
        let Some(text) = src.get(range.clone()) else {
            return;
        };
        let start = range.start;
        let mut ws = std::mem::take(&mut self.words);
        words(text, &mut ws);
        if let Some((polarity, verb, object, context)) = parse(&ws) {
            let md = self.md;
            let section = md.headings[..md
                .headings
                .partition_point(|h| h.span.start as usize <= start)]
                .last()
                .map(|h| h.text.as_str());
            self.out.push(Directive {
                polarity,
                verb,
                object,
                context,
                span: span(start, start + text.len()),
                section,
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

/// Byte length of a list marker (`- `, `* `, `+ `, `1. `, `2) `) at the start of `line`.
fn list_marker(line: &str) -> Option<usize> {
    let b = line.as_bytes();
    let digits = b.iter().take_while(|c| c.is_ascii_digit()).count();
    let len = match b {
        [b'-' | b'*' | b'+', b' ' | b'\t', ..] => 1,
        _ if (1..=9).contains(&digits) && matches!(b.get(digits), Some(b'.' | b')')) => digits + 1,
        _ => return None,
    };
    let ws = b[len..]
        .iter()
        .take_while(|c| matches!(c, b' ' | b'\t'))
        .count();
    (ws > 0).then_some(len + ws)
}

/// Whether one of `items` (sorted, non-overlapping spans) covers byte `pos`.
fn covers<T>(items: &[T], pos: usize, span: impl Fn(&T) -> Span) -> bool {
    let i = items.partition_point(|x| span(x).start as usize <= pos);
    i > 0 && span(&items[i - 1]).end as usize > pos
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
    }

    #[test]
    fn list_markers() {
        assert_eq!(list_marker("- a"), Some(2));
        assert_eq!(list_marker("12.  a"), Some(5));
        assert_eq!(list_marker("---"), None);
        assert_eq!(list_marker("1.5 x"), None);
    }
}
