//! AI-SEC-001: invisible or bidirectional Unicode.

use pulse_core::text::unicode::{is_emoji_base, is_invisible, is_tag, is_variation_selector};
use pulse_core::{Asset, Category, Finding, KindSet, Rule, RuleCtx, RuleMeta, Severity, Span};

use crate::util::truncate;

pub static META: RuleMeta = RuleMeta {
    id: "AI-SEC-001",
    name: "Invisible or bidirectional Unicode",
    category: Category::Security,
    default_severity: Severity::Critical,
    kinds: KindSet::ALL,
    summary: "File contains invisible, bidi-control or tag characters that hide text from reviewers.",
    remediation: "Remove the characters; nothing legitimate in an AI asset needs them.",
    inline_suppressible: false,
};

/// Longest decoded tag payload quoted in a finding.
const MAX_PAYLOAD: usize = 80;

pub struct InvisibleUnicode;

impl Rule for InvisibleUnicode {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, asset: &Asset, _ctx: &RuleCtx<'_>, out: &mut Vec<Finding>) {
        let src = asset.source.as_str();
        let Some(first) = first_candidate(src.as_bytes()) else {
            return;
        };
        let mut prev = src[..first]
            .chars()
            .next_back()
            .map_or(Base::None, Base::of);
        let mut run: Option<Run> = None;
        let mut chars = src[first..].char_indices().peekable();
        while let Some((i, c)) = chars.next() {
            let at = first + i;
            let next = chars.peek().map(|&(_, n)| n);
            if flagged(c, at, prev, next) {
                match &mut run {
                    Some(r) if r.end == at => r.push(c, at),
                    _ => {
                        if let Some(r) = run.take() {
                            r.report(asset, out);
                        }
                        run = Some(Run::new(c, at));
                    }
                }
                prev = Base::None;
            } else if is_invisible(c).is_none() {
                // An allowed variation selector or ZWJ keeps the emoji sequence going.
                prev = Base::of(c);
            }
        }
        if let Some(r) = run {
            r.report(asset, out);
        }
    }
}

/// Offset of the first UTF-8 lead byte that can start a code point of the set.
fn first_candidate(bytes: &[u8]) -> Option<usize> {
    let a = memchr::memchr3(0xE2, 0xEF, 0xF3, bytes);
    let b = memchr::memchr2(0xC2, 0xE1, bytes);
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

/// What the preceding code point allows after it (AI-SEC-001 FP guards).
#[derive(Clone, Copy, PartialEq)]
enum Base {
    None,
    /// `#`, `*`, `0`–`9`: a variation selector may follow (keycaps), a ZWJ may not.
    Keycap,
    Emoji,
}

impl Base {
    fn of(c: char) -> Self {
        match c {
            c if !is_emoji_base(c) => Base::None,
            c if c.is_ascii() => Base::Keycap,
            _ => Base::Emoji,
        }
    }
}

/// Whether `c` at byte offset `at` is reported, applying the spec's FP guards.
fn flagged(c: char, at: usize, prev: Base, next: Option<char>) -> bool {
    match c {
        '\u{FEFF}' => at != 0,
        '\u{200D}' => !(prev == Base::Emoji && next.is_some_and(|n| Base::of(n) == Base::Emoji)),
        c if is_variation_selector(c) => prev == Base::None,
        c => is_invisible(c).is_some(),
    }
}

/// A contiguous run of reported code points.
struct Run {
    start: usize,
    end: usize,
    first: char,
    mixed: bool,
    count: usize,
    tags: bool,
}

impl Run {
    fn new(c: char, at: usize) -> Self {
        Run {
            start: at,
            end: at + c.len_utf8(),
            first: c,
            mixed: false,
            count: 1,
            tags: is_tag(c),
        }
    }

    fn push(&mut self, c: char, at: usize) {
        self.end = at + c.len_utf8();
        self.mixed |= c != self.first;
        self.count += 1;
        self.tags |= is_tag(c);
    }

    fn report(&self, asset: &Asset, out: &mut Vec<Finding>) {
        let src = &asset.source[self.start..self.end];
        let message = if self.tags {
            let payload: String = src.chars().filter_map(tag_ascii).collect();
            format!(
                "Hidden tag text (×{}): \"{}\"",
                self.count,
                truncate(&payload, MAX_PAYLOAD)
            )
        } else {
            let name = is_invisible(self.first).unwrap_or("INVISIBLE CHARACTER");
            let others = if self.mixed { " and others" } else { "" };
            let count = if self.count > 1 {
                format!(" (×{})", self.count)
            } else {
                String::new()
            };
            format!(
                "Invisible character(s) U+{:04X} {name}{others}{count}",
                u32::from(self.first)
            )
        };
        let (Ok(start), Ok(end)) = (u32::try_from(self.start), u32::try_from(self.end)) else {
            return;
        };
        out.push(Finding::new(&META, asset, Span::new(start, end), message));
    }
}

/// The printable ASCII character a tag character (U+E0020–U+E007E) stands for.
fn tag_ascii(c: char) -> Option<char> {
    let b = u8::try_from(u32::from(c).checked_sub(0xE0000)?).ok()?;
    (0x20..0x7F).contains(&b).then_some(char::from(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_candidate_finds_earliest_lead_byte() {
        assert_eq!(first_candidate(b"plain ascii"), None);
        assert_eq!(first_candidate("ab\u{00AD}c\u{200B}".as_bytes()), Some(2));
        assert_eq!(first_candidate("a\u{E0041}".as_bytes()), Some(1));
    }

    #[test]
    fn tag_ascii_decodes_printable_only() {
        assert_eq!(tag_ascii('\u{E0041}'), Some('A'));
        assert_eq!(tag_ascii('\u{E0020}'), Some(' '));
        assert_eq!(tag_ascii('\u{E0001}'), None);
        assert_eq!(tag_ascii('\u{E007F}'), None);
        assert_eq!(tag_ascii('a'), None);
    }

    #[test]
    fn guards() {
        assert!(!flagged('\u{FEFF}', 0, Base::None, None));
        assert!(flagged('\u{FEFF}', 5, Base::None, None));
        assert!(!flagged('\u{200D}', 4, Base::Emoji, Some('👩')));
        assert!(flagged('\u{200D}', 4, Base::Emoji, Some('a')));
        assert!(flagged('\u{200D}', 4, Base::Keycap, Some('👩')));
        assert!(flagged('\u{200D}', 4, Base::None, Some('👩')));
        assert!(!flagged('\u{FE0F}', 4, Base::Emoji, None));
        assert!(!flagged('\u{FE0F}', 4, Base::Keycap, None));
        assert!(flagged('\u{FE0F}', 4, Base::None, None));
    }
}
