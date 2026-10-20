//! Prose-style vocabulary (AI-EFF-007, AI-COG-002, AI-COG-004). Words are lowercase unless
//! noted; comparisons ignore ASCII case. Multi-word entries are written as lemmas (no
//! plural `s`).

use std::sync::LazyLock;

use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};

use super::{expand, normalize_phrase};

/// Filler phrase templates and what to write instead (`""`: drop the phrase). Templates
/// expand like [`expand`]; the longest match wins (`make sure to always` → `always`).
pub const FILLER: &[(&str, &str)] = &[
    ("please note that", ""),
    ("{it is|it's} important to note {that|}", ""),
    ("{it is|it's} {important|essential|crucial} to", ""),
    ("it should be noted that", ""),
    ("make sure to always", "always"),
    ("make sure to", ""),
    ("make sure that", "ensure"),
    ("be sure to", ""),
    ("remember to always", "always"),
    ("you are a helpful {ai |}assistant", ""),
    ("as an ai{ language model| assistant|}", ""),
    ("in order to", "to"),
    ("feel free to", ""),
    ("basically", ""),
    ("very very", "very"),
    ("due to the fact that", "because"),
    ("at this point in time", "now"),
    ("in the event that", "if"),
    ("for all intents and purposes", ""),
    ("{with regard|with regards|in regard} to", "about"),
    ("a large number of", "many"),
    ("{do|try} your best to", ""),
    ("i want you to", ""),
    ("i would like you to", ""),
    ("needless to say", ""),
    ("it goes without saying that", ""),
    ("as previously mentioned", ""),
];

/// The compiled filler lexicon.
pub struct Filler {
    /// Matches every normalized expansion, leftmost-longest.
    pub automaton: AhoCorasick,
    /// Readable phrase of each pattern ID.
    pub display: Vec<String>,
    /// Replacement of each pattern ID (`""`: drop it).
    pub replacement: Vec<&'static str>,
}

/// [`FILLER`], expanded, normalized and compiled once.
#[allow(clippy::expect_used)] // static patterns (rust.md)
pub static FILLER_SET: LazyLock<Filler> = LazyLock::new(|| {
    let mut rows: Vec<(String, String, &'static str)> = FILLER
        .iter()
        .flat_map(|&(t, r)| expand(t).into_iter().map(move |d| (d, r)))
        .map(|(display, r)| (normalize_phrase(&display), display, r))
        .collect();
    rows.sort();
    rows.dedup_by(|a, b| a.0 == b.0);
    let automaton = AhoCorasickBuilder::new()
        .match_kind(MatchKind::LeftmostLongest)
        .ascii_case_insensitive(true)
        .build(rows.iter().map(|r| &r.0))
        .expect("static pattern");
    let (display, replacement) = rows.into_iter().map(|(_, d, r)| (d, r)).unzip();
    Filler {
        automaton,
        display,
        replacement,
    }
});

/// Vague terms (AI-COG-002), matched as whole lemmatized words.
pub const VAGUE: &[&str] = &[
    "appropriate",
    "appropriately",
    "as needed",
    "properly",
    "best practice",
    "clean code",
    "etc",
    "and so on",
    "some",
    "reasonable",
    "reasonably",
    "if necessary",
    "good",
    "nice",
];

/// Words that make a sentence concrete enough (AI-COG-002 anchor), besides digits, code
/// spans and paths.
pub const ANCHORS: &[&str] = &["e.g", "eg", "i.e", "example", "such as"];

/// Verbs that make a sentence a directive when they open a clause (`Write …`, `Keep …`),
/// beyond the AI-COG-001 markers and [`super::imperatives::IMPERATIVES`].
pub const DIRECTIVE_VERBS: &[&str] = &[
    "add",
    "apply",
    "avoid",
    "check",
    "comment",
    "document",
    "ensure",
    "format",
    "handle",
    "keep",
    "log",
    "make",
    "name",
    "prefer",
    "refactor",
    "structure",
    "test",
    "update",
    "validate",
    "verify",
];

/// Caps words that are emphasis markers whatever their length (AI-COG-004); matched
/// case-sensitively.
pub const EMPHASIS: &[&str] = &["IMPORTANT", "CRITICAL", "MUST", "NEVER"];

/// ALL-CAPS words of ≥ 4 letters that are names, not emphasis (AI-COG-004); matched
/// case-sensitively.
pub const ACRONYMS: &[&str] = &[
    "AGENTS",
    "AJAX",
    "ASCII",
    "CHANGELOG",
    "CLAUDE",
    "CORS",
    "CRUD",
    "CSRF",
    "CUDA",
    "DOCKER",
    "ESLINT",
    "GITHUB",
    "GITLAB",
    "JAVA",
    "NODE",
    "OPENAI",
    "PYTHON",
    "RUST",
    "STDERR",
    "STDIN",
    "STDOUT",
    "FIXME",
    "GEMINI",
    "GRPC",
    "HOME",
    "HTML",
    "HTMX",
    "HTTP",
    "HTTPS",
    "JSON",
    "JSONL",
    "LDAP",
    "LICENSE",
    "NULL",
    "OAUTH",
    "PATH",
    "POSIX",
    "README",
    "REST",
    "SAML",
    "SASS",
    "SCSS",
    "SKILL",
    "SPEC",
    "SQLITE",
    "TODO",
    "TOML",
    "UNIX",
    "UUID",
    "WASM",
    "XHTML",
    "YAML",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filler_expands_and_keeps_replacements() {
        let f = &*FILLER_SET;
        assert!(f.display.len() > FILLER.len());
        assert_eq!(f.display.len(), f.replacement.len());
        let at = |p: &str| f.display.iter().position(|d| d == p);
        assert_eq!(at("in order to").map(|i| f.replacement[i]), Some("to"));
        assert_eq!(
            at("it's important to note that").map(|i| f.replacement[i]),
            Some("")
        );
        let m = f.automaton.find("You should make sure to always run it");
        assert_eq!(
            m.map(|m| f.display[m.pattern().as_usize()].as_str()),
            Some("make sure to always")
        );
    }

    #[test]
    fn caps_lists_are_uppercase() {
        for w in EMPHASIS.iter().chain(ACRONYMS) {
            assert!(w.chars().all(|c| c.is_ascii_uppercase()), "{w}");
        }
    }
}
