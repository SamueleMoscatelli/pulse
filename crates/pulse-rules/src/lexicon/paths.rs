//! Sensitive file paths (AI-SEC-013; also AI-SEC-012 read targets) and deny-list wording.

use std::sync::LazyLock;

use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};

/// Path fragments of credential stores and private files, matched ASCII-case-insensitively
/// on raw text. `.env` is handled apart (see [`ENV`]).
pub const SENSITIVE_PATHS: &[&str] = &[
    ".ssh",
    "id_rsa",
    "id_dsa",
    "id_ecdsa",
    "id_ed25519",
    ".aws/credentials",
    "/etc/passwd",
    "/etc/shadow",
    "gh/hosts.yml",
    ".npmrc",
    ".pypirc",
    ".netrc",
    ".git-credentials",
    ".docker/config.json",
    ".kube/config",
    ".gnupg",
    "keychain",
    "keychains",
    "login.keychain-db",
    // Browser profile directories.
    "Google/Chrome/User Data",
    "Application Support/Google/Chrome",
    ".config/google-chrome",
    ".config/chromium",
    "BraveSoftware/Brave-Browser",
    "Microsoft/Edge/User Data",
    ".mozilla/firefox",
    "Firefox/Profiles",
    "Application Support/Firefox",
];

/// The dotenv file name; reported unless it names a template or sits in `.gitignore` advice.
pub const ENV: &str = ".env";

/// `.env.<suffix>` files that hold no secrets.
pub const ENV_TEMPLATES: &[&str] = &["example", "sample", "template", "dist", "defaults"];

/// Words on a line that make a `.env` mention `.gitignore` advice.
pub const GITIGNORE_WORDS: &[&str] = &["gitignore", "git ignore", "dockerignore"];

/// Lowercase phrases that make a sentence (or the lead-in of a list) a deny list.
pub const DENY_CONTEXT: &[&str] = &[
    "never read",
    "do not read",
    "don't read",
    "must not read",
    "never access",
    "do not access",
    "don't access",
    "must not access",
    "never open",
    "do not open",
    "don't open",
    "never touch",
    "do not touch",
    "don't touch",
    "never print",
    "do not print",
    "never commit",
    "do not commit",
    "don't commit",
    "never share",
    "do not share",
    "never send",
    "do not send",
    "never upload",
    "never expose",
    "do not expose",
    "forbidden",
    "off-limits",
    "off limits",
    "deny",
    "denied",
    "disallow",
];

/// [`SENSITIVE_PATHS`] then [`ENV`] (last pattern ID), leftmost-longest.
#[allow(clippy::expect_used)] // static patterns (rust.md)
pub static PATHS: LazyLock<AhoCorasick> = LazyLock::new(|| {
    AhoCorasickBuilder::new()
        .match_kind(MatchKind::LeftmostLongest)
        .ascii_case_insensitive(true)
        .build(SENSITIVE_PATHS.iter().chain([&ENV]))
        .expect("static pattern")
});

/// Pattern ID of [`ENV`] in [`PATHS`].
pub const ENV_ID: usize = SENSITIVE_PATHS.len();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_is_last_pattern() {
        let m = PATHS.find("cat .env").map(|m| m.pattern().as_usize());
        assert_eq!(m, Some(ENV_ID));
        assert!(DENY_CONTEXT.iter().all(|p| *p == p.to_lowercase()));
    }
}
