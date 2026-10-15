//! Shannon entropy, used to tell random-looking secrets from words.

/// Shannon entropy of `s` in bits per byte (bits per char for ASCII). Empty input is 0.
///
/// Single pass over the bytes, no allocation.
pub fn shannon_entropy(s: &str) -> f64 {
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return 0.0;
    }
    let mut counts = [0u32; 256];
    for &b in bytes {
        counts[usize::from(b)] += 1;
    }
    let n = bytes.len() as f64;
    counts
        .iter()
        .filter(|&&c| c > 0)
        .map(|&c| {
            let p = f64::from(c) / n;
            -p * p.log2()
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entropy_bounds() {
        assert_eq!(shannon_entropy(""), 0.0);
        assert_eq!(shannon_entropy("aaaa"), 0.0);
        assert!((shannon_entropy("abab") - 1.0).abs() < 1e-9);
        assert!((shannon_entropy("abcdefgh") - 3.0).abs() < 1e-9);
        assert!(shannon_entropy("password1234") < 3.5);
        assert!(shannon_entropy("q7Zp2LmX9vRk4TnB") > 3.5);
    }
}
