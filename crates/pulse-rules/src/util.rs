//! Small helpers shared by rules.

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_counts_chars() {
        assert_eq!(truncate("abc", 3), "abc");
        assert_eq!(truncate("abcd", 3), "ab…");
        assert_eq!(truncate("éééé", 3), "éé…");
        assert_eq!(truncate("", 0), "");
    }
}
