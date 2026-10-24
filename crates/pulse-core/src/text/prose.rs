//! Prose segmentation shared by the directive and style rules: the Markdown body split into
//! sentences, and sentences into words. [`Prose`] caches both per asset (P7.1): six rules
//! used to re-split and re-tokenize the same text.

use std::ops::Range;

use memchr::memchr;

use crate::model::Span;
use crate::parse::markdown::MarkdownView;

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
        // ASCII and U+FFFD (lossy-decoded bytes) first: the Unicode tables are slow (P7.1).
        let word = if c.is_ascii() {
            c.is_ascii_alphanumeric() || matches!(c, '_' | '+' | '#')
        } else {
            c != char::REPLACEMENT_CHARACTER && c.is_alphanumeric()
        };
        if word || joins {
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
    // Byte scan (P7.1): every delimiter is ASCII, and UTF-8 continuation bytes never are,
    // so no char decoding except for the char after a `.` `!` `?`.
    for (i, &b) in text.as_bytes().iter().enumerate() {
        match b {
            b'`' => code = !code,
            b'.' | b'!' | b'?'
                if !code
                    && text
                        .get(i + 1..)
                        .and_then(|rest| rest.chars().next())
                        .is_none_or(char::is_whitespace) =>
            {
                emit(unit.start + start..unit.start + i + 1);
                start = i + 1;
            }
            b';' | b'|' if !code => {
                emit(unit.start + start..unit.start + i);
                start = i + 1;
            }
            _ => {}
        }
    }
    emit(unit.start + start..unit.end);
}

/// Byte length of a list marker (`- `, `* `, `+ `, `1. `, `2) `) at the start of `line`.
pub fn list_marker(line: &str) -> Option<usize> {
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
pub fn covers<T>(items: &[T], pos: usize, span: impl Fn(&T) -> Span) -> bool {
    let i = items.partition_point(|x| span(x).start as usize <= pos);
    i > 0 && span(&items[i - 1]).end as usize > pos
}

/// An asset's sentences ([`sentences`]) with their words ([`words`]), as source offsets.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Prose {
    /// Byte range of each sentence, and the range of its words in `words`.
    sentences: Vec<(Range<u32>, Range<u32>)>,
    /// `(start, end, brk)` of every word.
    words: Vec<(u32, u32, bool)>,
}

/// One sentence of a [`Prose`].
#[derive(Debug, Clone)]
pub struct Sentence<'p> {
    /// Trimmed source byte range.
    pub range: Range<usize>,
    words: &'p [(u32, u32, bool)],
}

impl Prose {
    /// Segments `md`'s body in `src` (the source `md` was parsed from).
    pub fn new(src: &str, md: &MarkdownView) -> Self {
        let mut out = Self::default();
        let mut ws = Vec::new();
        sentences(src, md, |r| {
            let Some(text) = src.get(r.clone()) else {
                return;
            };
            words(text, &mut ws);
            let first = to_u32(out.words.len());
            out.words.extend(ws.iter().map(|w| {
                let start = r.start + (w.text.as_ptr() as usize - text.as_ptr() as usize);
                (to_u32(start), to_u32(start + w.text.len()), w.brk)
            }));
            let range = to_u32(r.start)..to_u32(r.end);
            out.sentences.push((range, first..to_u32(out.words.len())));
        });
        out
    }

    /// Sentences in source order.
    pub fn sentences(&self) -> impl ExactSizeIterator<Item = Sentence<'_>> {
        self.sentences.iter().map(|(r, w)| Sentence {
            range: r.start as usize..r.end as usize,
            words: self
                .words
                .get(w.start as usize..w.end as usize)
                .unwrap_or(&[]),
        })
    }
}

impl Sentence<'_> {
    /// The sentence's words, as slices of `src` (the asset source), into `out` (cleared).
    pub fn words<'a>(&self, src: &'a str, out: &mut Vec<Word<'a>>) {
        out.clear();
        out.extend(self.words.iter().filter_map(|&(s, e, brk)| {
            Some(Word {
                text: src.get(s as usize..e as usize)?,
                brk,
            })
        }));
    }
}

fn to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::LineIndex;

    #[test]
    fn prose_caches_sentences_and_words() {
        let src = "# T\n\n- Always use `a.b`; never (ever) skip it.\n\nCafé rules, done!\n";
        let md = MarkdownView::parse(src, &LineIndex::new(src));
        let prose = Prose::new(src, &md);
        let mut fresh = Vec::new();
        sentences(src, &md, |r| fresh.push(r));
        let cached: Vec<_> = prose.sentences().map(|s| s.range).collect();
        assert_eq!(cached, fresh);
        let (mut a, mut b) = (Vec::new(), Vec::new());
        for s in prose.sentences() {
            s.words(src, &mut a);
            words(&src[s.range.clone()], &mut b);
            assert!(
                a.len() == b.len()
                    && a.iter()
                        .zip(&b)
                        .all(|(x, y)| (x.text, x.brk) == (y.text, y.brk)),
                "{a:?} vs {b:?}"
            );
        }
        assert_eq!(prose.sentences().len(), 3);
    }

    #[test]
    fn list_markers() {
        assert_eq!(list_marker("- a"), Some(2));
        assert_eq!(list_marker("12.  a"), Some(5));
        assert_eq!(list_marker("---"), None);
        assert_eq!(list_marker("1.5 x"), None);
    }
}
