//! Secret patterns and placeholder words (AI-SEC-003).

use std::sync::{LazyLock, OnceLock};

use pulse_core::text::shannon_entropy;
use regex::{Regex, RegexSet};

/// One secret format. When the pattern has a capture group, group 1 is the secret.
pub struct Provider {
    pub name: &'static str,
    pub pattern: &'static str,
    /// Extra check for patterns that also match words or code.
    pub shape: Shape,
}

/// How random a match must look to be reported.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shape {
    /// The pattern alone is specific enough.
    Fixed,
    /// Upper case, lower case and a digit (rules out `sk-learn-pipeline-v2`).
    MixedCase,
    /// A generic assignment value: entropy, a digit, and not code.
    Assignment,
}

/// Minimum Shannon entropy (bits/char) of a generic assignment value.
pub const MIN_ENTROPY: f64 = 3.5;

const fn provider(name: &'static str, pattern: &'static str) -> Provider {
    Provider {
        name,
        pattern,
        shape: Shape::Fixed,
    }
}

/// Secret formats in precedence order: on overlapping matches the earlier entry wins
/// (Anthropic before OpenAI, every provider before the generic assignment).
pub const PROVIDERS: &[Provider] = &[
    provider("AWS", r"\bAKIA[0-9A-Z]{16}\b"),
    provider("GitHub", r"\bgh[pousr]_[A-Za-z0-9]{36,}"),
    provider("GitHub", r"\bgithub_pat_[A-Za-z0-9_]{22,}"),
    provider("Anthropic", r"\bsk-ant-[A-Za-z0-9_-]{20,}"),
    Provider {
        name: "OpenAI",
        pattern: r"\bsk-(?:proj-)?[A-Za-z0-9_-]{20,}",
        shape: Shape::MixedCase,
    },
    provider("Slack", r"\bxox[abprs]-[A-Za-z0-9-]{10,}"),
    provider("Google", r"\bAIza[0-9A-Za-z_-]{35}"),
    provider("Stripe", r"\bsk_live_[0-9A-Za-z]{20,}"),
    provider("private key", r"-----BEGIN (?:[A-Z0-9]+ )*PRIVATE KEY-----"),
    provider(
        "JWT",
        r"\beyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}",
    ),
    Provider {
        name: "generic",
        // `KEY = value`, `"apiKey": "value"`, `aws_secret_access_key: value`.
        pattern: r#"(?i)(?:api[_-]?key|token|secret|password)[a-z_]*["']?[ \t]*[:=][ \t]*["']?([^\s"']{12,})"#,
        shape: Shape::Assignment,
    },
];

/// The compiled prefilter, and per-pattern regexes in [`PROVIDERS`] order.
pub struct Matchers {
    pub set: RegexSet,
    /// Compiled on first use: only patterns the set matched need captures, and compiling
    /// all of them up front doubled the startup cost (P7.1).
    regexes: Vec<OnceLock<Regex>>,
}

impl Matchers {
    /// The regex of [`PROVIDERS`]`[i]`, compiled on first use.
    #[allow(clippy::expect_used)] // static patterns (rust.md)
    pub fn regex(&self, i: usize) -> Option<&Regex> {
        let (cell, provider) = (self.regexes.get(i)?, PROVIDERS.get(i)?);
        Some(cell.get_or_init(|| Regex::new(provider.pattern).expect("static pattern")))
    }
}

/// [`PROVIDERS`] compiled once (the set; regexes on demand).
#[allow(clippy::expect_used)] // static patterns (rust.md)
pub static MATCHERS: LazyLock<Matchers> = LazyLock::new(|| Matchers {
    set: RegexSet::new(PROVIDERS.iter().map(|p| p.pattern)).expect("static pattern"),
    regexes: PROVIDERS.iter().map(|_| OnceLock::new()).collect(),
});

/// Lowercase substrings that mark a value as a placeholder or a reference, not a secret.
pub const PLACEHOLDER_WORDS: &[&str] = &[
    "xxx",
    "your-",
    "your_",
    "example",
    "dummy",
    "changeme",
    "placeholder",
    "redacted",
    "<",
    "${",
    "{{",
    "process.env",
    "environ",
    "getenv",
];

/// Whether `secret` is an obvious placeholder: a placeholder word, an env reference
/// (`$X`, `${env:X}`), or mostly one repeated character.
pub fn is_placeholder(secret: &str) -> bool {
    if secret.starts_with('$') {
        return true;
    }
    let lower = secret.to_ascii_lowercase();
    PLACEHOLDER_WORDS.iter().any(|w| lower.contains(w)) || repetitive(secret)
}

/// Whether `secret` looks random enough for `shape` (AI-SEC-003 FP guard).
pub fn plausible(shape: Shape, secret: &str) -> bool {
    let has = |f: fn(&u8) -> bool| secret.as_bytes().iter().any(f);
    match shape {
        Shape::Fixed => true,
        Shape::MixedCase => {
            has(u8::is_ascii_uppercase) && has(u8::is_ascii_lowercase) && has(u8::is_ascii_digit)
        }
        Shape::Assignment => {
            has(u8::is_ascii_digit) && !code_like(secret) && shannon_entropy(secret) >= MIN_ENTROPY
        }
    }
}

/// Calls, indexing, dotted paths (`config.get_key()`, `settings.SECRET_KEY`), URLs and
/// word paths (`Desktop/Title/H1`; base64 segments like `K7MDENG` don't qualify).
fn code_like(s: &str) -> bool {
    s.contains(['(', ')', '[', ']'])
        || s.contains("://")
        || (s.contains('/') && s.split('/').all(word_segment))
        || (s.contains('.')
            && s.split('.').all(|part| {
                part.chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                    && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            }))
}

/// A word with an optional capital and trailing digits: `Desktop`, `H1`, `v2`.
fn word_segment(seg: &str) -> bool {
    let rest = seg
        .strip_prefix(|c: char| c.is_ascii_uppercase())
        .unwrap_or(seg);
    let digits = rest.trim_start_matches(|c: char| c.is_ascii_lowercase());
    digits.len() < seg.len() && digits.bytes().all(|b| b.is_ascii_digit())
}

/// The most frequent byte makes up at least half of `s` (e.g. `sk-aaaaaaaaaaaaaaaaaaaa`).
fn repetitive(s: &str) -> bool {
    let mut counts = [0usize; 256];
    for &b in s.as_bytes() {
        counts[usize::from(b)] += 1;
    }
    counts.iter().max().is_some_and(|&m| 2 * m >= s.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_patterns_compile() {
        let m = &*MATCHERS;
        assert_eq!(m.set.len(), PROVIDERS.len());
        assert!((0..PROVIDERS.len()).all(|i| m.regex(i).is_some()));
        assert!(m.regex(PROVIDERS.len()).is_none());
    }

    #[test]
    fn placeholders() {
        for p in [
            "sk-your-key-here-0123456789",
            "${env:OPENAI_API_KEY}",
            "$OPENAI_API_KEY",
            "<your token>",
            "AKIAIOSFODNN7EXAMPLE",
            "sk-aaaaaaaaaaaaaaaaaaaaaaaa",
            "changeme-please-now",
            "process.env.API_TOKEN",
            "{{ secrets.TOKEN }}",
        ] {
            assert!(is_placeholder(p), "{p}");
        }
        assert!(!is_placeholder("q7Zp2LmX9vRk4TnB8wYc"));
        assert!(!is_placeholder("-----BEGIN RSA PRIVATE KEY-----"));
    }

    #[test]
    fn plausible_rejects_words_and_code() {
        assert!(!plausible(
            Shape::MixedCase,
            "sk-learn-pipeline-transformer-v2"
        ));
        assert!(plausible(Shape::MixedCase, "sk-proj-q7Zp2LmX9vRk4TnB8wYc"));
        for code in [
            "AutoTokenizer.from_pretrained(",
            "config.get_api_key()",
            "settings.SECRET_KEY2",
            "os.environ[KEY]",
            "rotatedmonthly",
            "https://login.example.org/oauth2/v2.0/token",
            "Desktop/Title/H1",
        ] {
            assert!(!plausible(Shape::Assignment, code), "{code}");
        }
        assert!(plausible(Shape::Assignment, "q7Zp2LmX9vRk4TnB"));
        assert!(plausible(Shape::Assignment, "3f9a1c07be524d8e"));
        // Base64 with slashes is not a word path.
        assert!(plausible(
            Shape::Assignment,
            "wJalrXUtnFEMI/K7MDENG/bPxRfiCY9z"
        ));
    }
}
