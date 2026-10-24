//! Text normalization for obfuscation-resistant matching, with an offset map back to source.

use std::ops::Range;

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::canonical_combining_class;

use super::unicode::is_invisible;
use crate::model::Span;

/// Normalized text plus, for every normalized byte, the source byte offset it came from.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Normalized {
    /// Normalized text: invisibles dropped, NFKC, confusables folded, lowercased,
    /// whitespace collapsed, Markdown emphasis markers stripped.
    pub text: String,
    /// `map[i]` is the source byte offset of normalized byte `i`. Non-decreasing, with
    /// one trailing sentinel equal to the source length (`map.len() == text.len() + 1`).
    pub map: Vec<u32>,
}

impl Normalized {
    /// Source span covering the normalized byte range `norm` (clamped to the text).
    ///
    /// The end extends to the end of the last source unit touched, so a range inside a
    /// NFKC expansion (e.g. the `f` of `ﬁ`) still maps to the whole source char.
    pub fn source_span(&self, norm: Range<usize>) -> Span {
        let len = self.text.len();
        let start = norm.start.min(len);
        let end = norm.end.clamp(start, len);
        let src_start = self.map[start];
        if start == end {
            return Span::new(src_start, src_start);
        }
        let last = self.map[end - 1];
        // The sentinel (source length) is strictly greater than any char start, so this ends.
        let mut j = end;
        while self.map[j] <= last {
            j += 1;
        }
        Span::new(src_start, self.map[j])
    }
}

/// Normalizes `src` as the spec's "Text normalization" section describes.
pub fn normalize(src: &str) -> Normalized {
    let mut out = Builder {
        text: String::with_capacity(src.len()),
        map: Vec::with_capacity(src.len() + 1),
    };
    // Pending NFKC segment: a starter plus its trailing non-starters.
    let mut seg: Vec<char> = Vec::new();
    let mut seg_start = 0u32;

    let bytes = src.as_bytes();
    let mut i = 0;
    while let Some(&b) = bytes.get(i) {
        let off = to_u32(i);
        // Fast path (P7.1): an ASCII byte followed by ASCII (or the end) is a complete NFKC
        // segment, not invisible and not a confusable: no segment buffering or NFKC pass.
        if b.is_ascii() && bytes.get(i + 1).is_none_or(u8::is_ascii) {
            out.flush(&mut seg, seg_start);
            out.push_ascii(b, off);
            i += 1;
            continue;
        }
        let Some(c) = src.get(i..).and_then(|rest| rest.chars().next()) else {
            break;
        };
        i += c.len_utf8();
        if is_invisible(c).is_some() {
            continue;
        }
        if c.is_ascii() || canonical_combining_class(c) == 0 {
            out.flush(&mut seg, seg_start);
            seg_start = off;
            seg.push(c);
        } else {
            if seg.is_empty() {
                seg_start = off;
            }
            seg.push(c);
        }
    }
    out.flush(&mut seg, seg_start);
    out.map.push(to_u32(src.len()));
    Normalized {
        text: out.text,
        map: out.map,
    }
}

struct Builder {
    text: String,
    map: Vec<u32>,
}

impl Builder {
    /// NFKC-normalizes the pending segment and pushes the result, all mapped to `src`.
    fn flush(&mut self, seg: &mut Vec<char>, src: u32) {
        match seg.as_slice() {
            [] => return,
            // Fast path: a lone ASCII char is already NFKC.
            [c] if c.is_ascii() => self.push(*c, src),
            _ => {
                for c in seg.iter().copied().nfkc() {
                    self.push(c, src);
                }
            }
        }
        seg.clear();
    }

    /// Applies steps 3–4 (confusables, lowercase, whitespace, emphasis) to one NFKC char.
    fn push(&mut self, c: char, src: u32) {
        let c = confusable(c).unwrap_or(c);
        if c.is_whitespace() {
            if !self.text.ends_with(' ') {
                self.emit(' ', src);
            }
        } else if matches!(c, '*' | '_' | '~' | '`') {
            // Markdown emphasis markers carry no meaning for matching.
        } else if c.is_ascii() {
            self.emit(c.to_ascii_lowercase(), src);
        } else {
            for l in c.to_lowercase() {
                self.emit(l, src);
            }
        }
    }

    /// [`Self::push`] for an ASCII byte.
    fn push_ascii(&mut self, b: u8, src: u32) {
        if (b as char).is_whitespace() {
            if !self.text.ends_with(' ') {
                self.text.push(' ');
                self.map.push(src);
            }
        } else if !matches!(b, b'*' | b'_' | b'~' | b'`') {
            self.text.push(char::from(b.to_ascii_lowercase()));
            self.map.push(src);
        }
    }

    fn emit(&mut self, c: char, src: u32) {
        self.text.push(c);
        self.map.extend(std::iter::repeat_n(src, c.len_utf8()));
    }
}

/// Latin lookalike for common Cyrillic/Greek confusables (already lowercased).
fn confusable(c: char) -> Option<char> {
    let l = match c {
        // Cyrillic
        'а' | 'А' => 'a',
        'В' => 'b',
        'с' | 'С' => 'c',
        'ԁ' => 'd',
        'е' | 'Е' => 'e',
        'Н' => 'h',
        'і' | 'І' => 'i',
        'ј' | 'Ј' => 'j',
        'К' => 'k',
        'М' => 'm',
        'о' | 'О' => 'o',
        'р' | 'Р' => 'p',
        'ѕ' | 'Ѕ' => 's',
        'Т' => 't',
        'у' => 'y',
        'х' | 'Х' => 'x',
        // Greek
        'α' | 'Α' => 'a',
        'Β' => 'b',
        'Ε' => 'e',
        'Η' => 'h',
        'ι' | 'Ι' => 'i',
        'κ' | 'Κ' => 'k',
        'Μ' => 'm',
        'Ν' => 'n',
        'ο' | 'Ο' => 'o',
        'ρ' | 'Ρ' => 'p',
        'Τ' => 't',
        'υ' => 'u',
        'ν' => 'v',
        'Χ' => 'x',
        'Υ' => 'y',
        'Ζ' => 'z',
        _ => return None,
    };
    Some(l)
}

fn to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn src_of<'a>(src: &'a str, n: &Normalized, norm: Range<usize>) -> &'a str {
        let s = n.source_span(norm);
        &src[s.start as usize..s.end as usize]
    }

    #[test]
    fn normalize_folds_full_width() {
        assert_eq!(normalize("ｉｇｎｏｒｅ").text, "ignore");
    }

    #[test]
    fn ascii_before_combining_mark_still_composes() {
        let n = normalize("Cafe\u{301} x\u{308}\u{301}Y\u{0}\u{B}z");
        assert_eq!(n.text, "café ẍ\u{301}y\u{0} z");
        // `é` (2 bytes) maps to the `e` it was composed from.
        assert_eq!(&n.map[3..5], &[3, 3]);
        assert_eq!(n.map.len(), n.text.len() + 1);
    }

    #[test]
    fn normalize_folds_cyrillic_confusables() {
        assert_eq!(normalize("іgnore").text, "ignore");
        assert_eq!(normalize("ΙGΝΟRΕ рrеviоus").text, "ignore previous");
    }

    #[test]
    fn normalize_lowercases_and_collapses_whitespace() {
        let n = normalize("Ignore \t\n  ALL\u{00A0}previous");
        assert_eq!(n.text, "ignore all previous");
    }

    #[test]
    fn normalize_strips_emphasis_markers() {
        assert_eq!(
            normalize("**ignore** _all_ ~~x~~ `y`").text,
            "ignore all x y"
        );
    }

    #[test]
    fn normalize_drops_invisibles_and_tags() {
        let src = format!("ig\u{200B}no\u{202E}re{}", "\u{E0041}\u{E0042}");
        let n = normalize(&src);
        assert_eq!(n.text, "ignore");
        // Trailing dropped chars fold into the span of the last kept char.
        assert_eq!(src_of(&src, &n, 0..6), src);
        assert_eq!(src_of(&src, &n, 0..1), "i");
        assert_eq!(src_of(&src, &n, 2..4), "no\u{202E}");
    }

    #[test]
    fn map_has_sentinel_and_is_monotonic() {
        let src = "Ａ b\u{200B}ﬁ";
        let n = normalize(src);
        assert_eq!(n.map.len(), n.text.len() + 1);
        assert_eq!(*n.map.last().unwrap() as usize, src.len());
        assert!(n.map.windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn offsets_survive_nfkc_expansion() {
        let src = "a ﬁle";
        let n = normalize(src);
        assert_eq!(n.text, "a file");
        assert_eq!(src_of(src, &n, 2..6), "ﬁle");
        // A range inside the expansion maps to the whole ligature.
        assert_eq!(src_of(src, &n, 2..3), "ﬁ");
        assert_eq!(src_of(src, &n, 4..6), "le");
    }

    #[test]
    fn offsets_survive_nfkc_contraction() {
        let src = "cafe\u{0301} ok";
        let n = normalize(src);
        assert_eq!(n.text, "café ok");
        let e = n.text.find('é').unwrap();
        assert_eq!(src_of(src, &n, e..e + 'é'.len_utf8()), "e\u{0301}");
        let ok = n.text.find("ok").unwrap();
        assert_eq!(src_of(src, &n, ok..ok + 2), "ok");
    }

    #[test]
    fn offsets_survive_full_width_and_whitespace_collapse() {
        let src = "say   ｉｇｎｏｒｅ\n\nnow";
        let n = normalize(src);
        assert_eq!(n.text, "say ignore now");
        assert_eq!(src_of(src, &n, 4..10), "ｉｇｎｏｒｅ");
        assert_eq!(src_of(src, &n, 3..4), "   ");
        assert_eq!(src_of(src, &n, 11..14), "now");
    }

    #[test]
    fn source_span_clamps_and_handles_empty() {
        let n = normalize("abc");
        assert_eq!(n.source_span(1..1), Span::new(1, 1));
        assert_eq!(n.source_span(2..99), Span::new(2, 3));
        assert_eq!(normalize("").source_span(0..5), Span::new(0, 0));
    }

    #[test]
    fn leading_combining_mark_is_kept() {
        let n = normalize("\u{0301}x");
        assert_eq!(n.text, "\u{0301}x");
        assert_eq!(n.map, vec![0, 0, 2, 3]);
    }
}
