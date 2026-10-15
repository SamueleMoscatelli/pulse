//! Shared word lists and the automata built from them.

pub mod directives;
pub mod injection;
pub mod mcp;
pub mod permissions;
pub mod secrets;
pub mod shell;

use pulse_core::text::normalize;

/// Every expansion of `template`, where `{a|b|}` picks one alternative (possibly empty).
///
/// Templates are trusted constants; an unclosed `{` is kept literally.
pub fn expand(template: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        let Some(len) = rest[open..].find('}') else {
            break;
        };
        let head = &rest[..open];
        let alts: Vec<&str> = rest[open + 1..open + len].split('|').collect();
        out = out
            .iter()
            .flat_map(|prefix| alts.iter().map(move |alt| format!("{prefix}{head}{alt}")))
            .collect();
        rest = &rest[open + len + 1..];
    }
    for s in &mut out {
        s.push_str(rest);
    }
    out
}

/// `phrase` in the form the matchers see: normalized like asset text and trimmed.
pub fn normalize_phrase(phrase: &str) -> String {
    normalize(phrase).text.trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_cartesian_with_empty_alternative() {
        assert_eq!(
            expand("ignore {all |}{prior|previous} rules"),
            [
                "ignore all prior rules",
                "ignore all previous rules",
                "ignore prior rules",
                "ignore previous rules",
            ]
        );
        assert_eq!(expand("plain"), ["plain"]);
        assert_eq!(expand("open {brace"), ["open {brace"]);
    }

    #[test]
    fn normalize_phrase_matches_asset_normalization() {
        assert_eq!(normalize_phrase("<|im_start|>"), "<|imstart|>");
        assert_eq!(normalize_phrase(" Do  NOT tell "), "do not tell");
    }
}
