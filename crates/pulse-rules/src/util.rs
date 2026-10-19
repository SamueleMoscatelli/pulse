//! Small helpers shared by rules.

use pulse_core::text::levenshtein_within;

/// `s` cut to at most `max` chars, with `…` appended when cut.
pub fn truncate(s: &str, max: usize) -> std::borrow::Cow<'_, str> {
    match s.char_indices().nth(max) {
        None => s.into(),
        Some((i, _)) => {
            let cut = s
                .char_indices()
                .nth(max.saturating_sub(1))
                .map_or(i, |(j, _)| j);
            format!("{}…", s[..cut].trim_end()).into()
        }
    }
}

/// The last `max` chars of `s`, with a leading `…` when cut.
pub fn tail(s: &str, max: usize) -> std::borrow::Cow<'_, str> {
    let n = s.chars().count();
    if n <= max {
        return s.into();
    }
    let start = s.char_indices().nth(n - max + 1).map_or(0, |(i, _)| i);
    format!("…{}", &s[start..]).into()
}

/// Levenshtein distance between `a` and `b` (ASCII case-insensitive) when at most `max`.
pub fn distance_within(a: &str, b: &str, max: usize) -> Option<usize> {
    levenshtein_within(&a.to_ascii_lowercase(), &b.to_ascii_lowercase(), max)
}

/// The closest of `known` to `word` within a third of its length (at least 1, at most 3
/// edits), ignoring ASCII case; ties keep the first in `known`.
pub fn suggest<'a>(word: &str, known: &[&'a str]) -> Option<&'a str> {
    let max = (word.chars().count() / 3).clamp(1, 3);
    known
        .iter()
        .filter_map(|k| Some((distance_within(word, k, max)?, *k)))
        .min_by_key(|&(d, _)| d)
        .map(|(_, k)| k)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_keeps_the_end() {
        assert_eq!(tail("abc", 5), "abc");
        assert_eq!(tail("abcdefgh", 5), "…efgh");
    }

    #[test]
    fn distance_ignores_ascii_case() {
        assert_eq!(distance_within("Reed", "Read", 1), Some(1));
        assert_eq!(distance_within("read", "Read", 0), Some(0));
        assert_eq!(distance_within("kitten", "sitting", 2), None);
    }

    #[test]
    fn suggest_picks_closest_within_budget() {
        let known = ["Read", "Write", "WebFetch"];
        assert_eq!(suggest("Reed", &known), Some("Read"));
        assert_eq!(suggest("WebFech", &known), Some("WebFetch"));
        assert_eq!(suggest("Deploy", &known), None);
    }

    #[test]
    fn truncate_counts_chars() {
        assert_eq!(truncate("abc", 3), "abc");
        assert_eq!(truncate("abcd", 3), "ab…");
        assert_eq!(truncate("éééé", 3), "éé…");
        assert_eq!(truncate("", 0), "");
    }
}
