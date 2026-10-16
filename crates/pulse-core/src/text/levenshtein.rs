//! Bounded edit distance, for typo hints.

/// Levenshtein distance between `a` and `b` (in chars) when it is at most `max`, else `None`.
/// Exits early on a length gap over `max` or once a whole row exceeds it.
pub fn levenshtein_within(a: &str, b: &str, max: usize) -> Option<usize> {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.len().abs_diff(b.len()) > max {
        return None;
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for (i, &ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        let mut row_min = cur[0];
        for (j, &cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1);
            row_min = row_min.min(cur[j + 1]);
        }
        if row_min > max {
            return None;
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    Some(prev[b.len()]).filter(|&d| d <= max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distances_within_bound() {
        assert_eq!(levenshtein_within("descripton", "description", 2), Some(1));
        assert_eq!(levenshtein_within("alwaysapply", "alwaysApply", 2), Some(1));
        assert_eq!(levenshtein_within("glob", "globs", 2), Some(1));
        assert_eq!(levenshtein_within("same", "same", 0), Some(0));
        assert_eq!(levenshtein_within("", "ab", 2), Some(2));
        assert_eq!(levenshtein_within("kitten", "sitting", 3), Some(3));
        assert_eq!(levenshtein_within("é", "e", 1), Some(1));
    }

    #[test]
    fn over_bound_is_none() {
        assert_eq!(levenshtein_within("kitten", "sitting", 2), None);
        assert_eq!(levenshtein_within("a", "abcd", 2), None);
        assert_eq!(levenshtein_within("model", "tools", 2), None);
    }
}
