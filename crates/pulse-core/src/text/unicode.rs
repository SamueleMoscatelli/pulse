//! Invisible / format code points (the AI-SEC-001 set) and an approximate emoji test.

/// Name of `c` if it belongs to the AI-SEC-001 invisible set, else `None`.
///
/// Covers every code point in the set unconditionally; context-dependent exemptions
/// (U+FEFF at offset 0, variation selectors after an emoji base, ZWJ inside emoji
/// sequences) are the caller's job.
pub fn is_invisible(c: char) -> Option<&'static str> {
    let name = match c {
        '\u{00AD}' => "SOFT HYPHEN",
        '\u{180E}' => "MONGOLIAN VOWEL SEPARATOR",
        '\u{200B}' => "ZERO WIDTH SPACE",
        '\u{200C}' => "ZERO WIDTH NON-JOINER",
        '\u{200D}' => "ZERO WIDTH JOINER",
        '\u{200E}' => "LEFT-TO-RIGHT MARK",
        '\u{200F}' => "RIGHT-TO-LEFT MARK",
        '\u{202A}' => "LEFT-TO-RIGHT EMBEDDING",
        '\u{202B}' => "RIGHT-TO-LEFT EMBEDDING",
        '\u{202C}' => "POP DIRECTIONAL FORMATTING",
        '\u{202D}' => "LEFT-TO-RIGHT OVERRIDE",
        '\u{202E}' => "RIGHT-TO-LEFT OVERRIDE",
        '\u{2060}' => "WORD JOINER",
        '\u{2061}' => "FUNCTION APPLICATION",
        '\u{2062}' => "INVISIBLE TIMES",
        '\u{2063}' => "INVISIBLE SEPARATOR",
        '\u{2064}' => "INVISIBLE PLUS",
        '\u{2066}' => "LEFT-TO-RIGHT ISOLATE",
        '\u{2067}' => "RIGHT-TO-LEFT ISOLATE",
        '\u{2068}' => "FIRST STRONG ISOLATE",
        '\u{2069}' => "POP DIRECTIONAL ISOLATE",
        '\u{FEFF}' => "ZERO WIDTH NO-BREAK SPACE",
        '\u{FE00}'..='\u{FE0F}' | '\u{E0100}'..='\u{E01EF}' => "VARIATION SELECTOR",
        '\u{E0001}' => "LANGUAGE TAG",
        '\u{E007F}' => "CANCEL TAG",
        '\u{E0000}'..='\u{E007E}' => "TAG CHARACTER",
        _ => return None,
    };
    Some(name)
}

/// Whether `c` is a tag character (U+E0000–U+E007F, "ASCII smuggling").
pub fn is_tag(c: char) -> bool {
    matches!(c, '\u{E0000}'..='\u{E007F}')
}

/// Whether `c` is a variation selector (U+FE00–U+FE0F, U+E0100–U+E01EF).
pub fn is_variation_selector(c: char) -> bool {
    matches!(c, '\u{FE00}'..='\u{FE0F}' | '\u{E0100}'..='\u{E01EF}')
}

/// Approximate test for a code point that can start or continue an emoji sequence.
///
/// Used by the AI-SEC-001 ZWJ / variation-selector guards; over-inclusive by design
/// (whole pictograph blocks, plus keycap bases `#`, `*`, `0`–`9`).
pub fn is_emoji_base(c: char) -> bool {
    matches!(
        c,
        '#' | '*'
            | '0'..='9'
            | '\u{00A9}'
            | '\u{00AE}'
            | '\u{203C}'
            | '\u{2049}'
            | '\u{2122}'
            | '\u{2139}'
            | '\u{2194}'..='\u{21AA}'
            | '\u{231A}'..='\u{23FF}'
            | '\u{24C2}'
            | '\u{25AA}'..='\u{25FE}'
            | '\u{2600}'..='\u{27BF}'
            | '\u{2934}'..='\u{2935}'
            | '\u{2B05}'..='\u{2B55}'
            | '\u{3030}'
            | '\u{303D}'
            | '\u{3297}'
            | '\u{3299}'
            | '\u{1F000}'..='\u{1FAFF}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invisible_covers_the_ai_sec_001_set() {
        let ranges: &[(u32, u32)] = &[
            (0x200B, 0x200F),
            (0x202A, 0x202E),
            (0x2060, 0x2064),
            (0x2066, 0x2069),
            (0xFEFF, 0xFEFF),
            (0x00AD, 0x00AD),
            (0x180E, 0x180E),
            (0xE0000, 0xE007F),
            (0xFE00, 0xFE0F),
            (0xE0100, 0xE01EF),
        ];
        for &(lo, hi) in ranges {
            for cp in lo..=hi {
                let c = char::from_u32(cp).unwrap();
                assert!(is_invisible(c).is_some(), "U+{cp:04X} not covered");
            }
        }
    }

    #[test]
    fn invisible_names_are_specific() {
        assert_eq!(is_invisible('\u{202E}'), Some("RIGHT-TO-LEFT OVERRIDE"));
        assert_eq!(is_invisible('\u{200B}'), Some("ZERO WIDTH SPACE"));
        assert_eq!(is_invisible('\u{E0041}'), Some("TAG CHARACTER"));
    }

    #[test]
    fn invisible_excludes_ordinary_text() {
        for c in [
            'a', ' ', '\n', '\t', 'é', '日', '\u{2065}', '\u{2010}', '😀',
        ] {
            assert_eq!(is_invisible(c), None, "{c:?}");
        }
    }

    #[test]
    fn tag_and_variation_selector_predicates() {
        assert!(is_tag('\u{E0020}') && !is_tag('\u{E0080}'));
        assert!(is_variation_selector('\u{FE0F}') && is_variation_selector('\u{E0100}'));
        assert!(!is_variation_selector('\u{FE10}'));
    }

    #[test]
    fn emoji_base_approximates_pictographs() {
        for c in ['😀', '👍', '❤', '☀', '🇮', '1', '#', '©'] {
            assert!(is_emoji_base(c), "{c:?}");
        }
        for c in ['a', ' ', 'é', '日', '\u{200D}'] {
            assert!(!is_emoji_base(c), "{c:?}");
        }
    }
}
