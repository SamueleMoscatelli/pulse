//! Tool-description poisoning vocabulary (AI-SEC-012).

use std::sync::LazyLock;

use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};
use regex::Regex;

use super::{expand, normalize_phrase};

/// Phrase templates that ask the model to hide its behavior from the user; expanded and
/// normalized like the injection lexicon.
pub const CONCEALMENT: &[&str] = &[
    "do not mention",
    "don't mention",
    "never mention",
    "do not tell the user",
    "don't tell the user",
    "never tell the user",
    "do not inform the user",
    "do not notify the user",
    "without {telling|informing|notifying|alerting|asking} the user",
    "without the user {knowing|noticing}",
    "the user {must|should} not {know|see|notice}",
    "{hide|conceal} {this|it} from the user",
    "keep this {secret|hidden|private}",
    "do not reveal this",
    "{silently|secretly|quietly} {call|invoke|run|read|send|pass|include|upload}",
    "this is an implementation detail",
];

/// Readable form and compiled automaton of [`CONCEALMENT`] (normalized, leftmost-longest).
pub struct Concealment {
    pub automaton: AhoCorasick,
    pub display: Vec<String>,
}

#[allow(clippy::expect_used)] // static patterns (rust.md)
pub static CONCEAL: LazyLock<Concealment> = LazyLock::new(|| {
    let display: Vec<String> = CONCEALMENT.iter().flat_map(|t| expand(t)).collect();
    let normalized: Vec<String> = display.iter().map(|d| normalize_phrase(d)).collect();
    let automaton = AhoCorasickBuilder::new()
        .match_kind(MatchKind::LeftmostLongest)
        .ascii_case_insensitive(true)
        .build(&normalized)
        .expect("static pattern");
    Concealment { automaton, display }
});

/// `<IMPORTANT>`-style tags (opening or closing) used to smuggle instructions to the model.
#[allow(clippy::expect_used)] // static pattern (rust.md)
pub static HIDDEN_TAG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)<\s*/?\s*(?:important|system|instructions?|hidden|secret|admin|critical|override|assistant)(?:\s[^>]*)?>",
    )
    .expect("static pattern")
});

/// Agent and MCP client config locations (lowercase) whose files hold server credentials;
/// reading them is a poisoning target besides the AI-SEC-013 paths.
pub const AGENT_CONFIGS: &[&str] = &[
    "mcp.json",
    "~/.cursor",
    "~/.claude",
    "~/.codeium",
    "~/.gemini",
    "~/.continue",
    "claude_desktop_config.json",
];

/// Verbs that read a file or move its contents (Detect: read files or credentials).
pub const READ_VERBS: &[&str] = &[
    "read", "cat", "open", "load", "access", "dump", "extract", "collect", "grab", "pass", "send",
    "include", "attach", "upload", "forward", "provide", "copy", "append", "embed",
];

/// Verbs that move data into a call; with a credential word they ask the model to leak it.
pub const MOVE_VERBS: &[&str] = &[
    "pass", "send", "include", "attach", "upload", "forward", "provide", "copy", "append", "embed",
];

/// Credential words (lemmas) that are a leak target after a [`MOVE_VERBS`] verb.
pub const CREDENTIALS: &[&str] = &[
    "credential",
    "private key",
    "ssh key",
    "api key",
    "apikey",
    "password",
    "passphrase",
    "secret",
    "access token",
    "auth token",
    "session cookie",
];

/// An instruction to call another tool (`you must first call the \`send\` tool`).
#[allow(clippy::expect_used)] // static pattern (rust.md)
pub static TOOL_CALL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:must|always|first|also|then|immediately)\s+(?:call|invoke|run|use|execute|trigger)\s+(?:the\s+)?`?[\w.-]+`?\s+tool\b|\b(?:call|invoke|use)\s+(?:the\s+)?`?[\w.-]+`?\s+(?:tool\s+)?instead\s+of\s+(?:this|the)\b",
    )
    .expect("static pattern")
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_compile_and_match() {
        assert!(HIDDEN_TAG.is_match("<IMPORTANT>"));
        assert!(HIDDEN_TAG.is_match("</ important >"));
        assert!(!HIDDEN_TAG.is_match("<importantly>"));
        assert!(TOOL_CALL.is_match("you must first call the `send_mail` tool"));
        assert!(TOOL_CALL.is_match("Use `safe_add` instead of this tool"));
        assert!(!TOOL_CALL.is_match("Use this tool to add numbers"));
        let n = CONCEAL.automaton.find("ok. do not mention this.").is_some();
        assert!(n);
    }
}
