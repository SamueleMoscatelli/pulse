//! Prompt-injection phrases (AI-SEC-002).

use std::sync::LazyLock;

use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};

use super::{expand, normalize_phrase};

/// Phrase templates; `{a|b|}` alternatives are expanded at build time (see [`expand`]).
pub const PHRASES: &[&str] = &[
    // Override the earlier context.
    "ignore {all |any |the |}{previous|prior|above|preceding|earlier} {instructions|rules|directions|prompts|guidelines|messages}",
    "ignore {all |any |}your {instructions|rules|guidelines|programming|system prompt}",
    "ignore all instructions",
    "ignore {your|all|the} safety {guidelines|rules|filters}",
    "disregard {all |any |the |}{previous|prior|above|preceding|earlier} {instructions|rules|directions|prompts|guidelines}",
    "disregard the above",
    "disregard {all |}your {instructions|rules|guidelines|programming}",
    "forget {all |}{previous|prior|above|your} {instructions|rules|guidelines}",
    "forget everything",
    "override {all |}{previous|prior|your} {instructions|rules}",
    "stop following your {instructions|rules|guidelines}",
    "do not follow your {instructions|rules|guidelines}",
    "the above instructions are {void|cancelled|obsolete}",
    "system override",
    "admin override",
    "end of system prompt",
    "begin new instructions",
    "new instructions:",
    "updated instructions:",
    "your new {task|instructions} {is|are}",
    "your {real|true} {task|instructions|purpose} {is|are}",
    // Persona and mode switches.
    "you are now in {developer|dev|god|jailbreak|unrestricted|debug|admin|sudo} mode",
    "{enable|enter|activate} {developer|god|jailbreak|unrestricted} mode",
    "developer mode enabled",
    "jailbreak mode",
    // A bare "sudo mode" is also a product feature (re-authentication before sensitive
    // actions): only the switch addressed to the model counts.
    "sudo mode {enabled|activated}",
    "unfiltered mode",
    "act as dan",
    "you are dan",
    "do anything now",
    "you {have|are under} no {restrictions|rules|limitations|guidelines}",
    "pretend you have no {restrictions|rules|limitations|guidelines}",
    "you are no longer bound by",
    "respond without {any |}{restrictions|filters|limitations}",
    "bypass {your|all|any} {safety|content} {filters|guidelines|restrictions}",
    // Hiding things from the user.
    "do not tell the user",
    "don't tell the user",
    "do not inform the user",
    "without telling the user",
    "do not reveal this to the user",
    "hide this from the user",
    "keep this secret from the user",
    "do not mention this {instruction|message|prompt}",
    // Prompt leaking.
    "{reveal|print|repeat|output} your system prompt",
    "{reveal|print|repeat|output} your instructions verbatim",
    // Chat-template and role-marker tokens.
    "</system>",
    "<|im_start|>",
    "<|im_end|>",
    "<|system|>",
    "<|endoftext|>",
    "<<sys>>",
    "[[system]]",
    "[system message]",
    "[/inst]",
];

/// Phrases that are injections only when addressed to the model: they open their clause,
/// possibly after [`LEADERS`](super::imperatives::LEADERS) (`Enter sudo mode`, `You must
/// enable sudo mode`), not `ask the user to enter sudo mode` (a product feature).
pub const ADDRESSED: &[&str] = &["{enable|enter|activate} sudo mode"];

/// The compiled injection lexicon.
pub struct Injection {
    /// Matches every normalized expansion, leftmost-longest.
    pub automaton: AhoCorasick,
    /// Normalized phrase of each pattern ID (for allow lists).
    pub normalized: Vec<String>,
    /// Readable phrase of each pattern ID (for messages).
    pub display: Vec<String>,
    /// Whether each pattern ID is from [`ADDRESSED`].
    pub addressed: Vec<bool>,
}

/// [`PHRASES`], expanded, normalized and compiled once.
#[allow(clippy::expect_used)] // static patterns (rust.md)
pub static INJECTION: LazyLock<Injection> = LazyLock::new(|| {
    let tagged = |list: &'static [&'static str], addressed: bool| {
        list.iter()
            .flat_map(|t| expand(t))
            .map(move |display| (normalize_phrase(&display), display, addressed))
    };
    let mut triples: Vec<(String, String, bool)> = tagged(PHRASES, false)
        .chain(tagged(ADDRESSED, true))
        .collect();
    triples.sort();
    triples.dedup_by(|a, b| a.0 == b.0);
    let mut normalized = Vec::with_capacity(triples.len());
    let mut display = Vec::with_capacity(triples.len());
    let mut addressed = Vec::with_capacity(triples.len());
    for (n, d, a) in triples {
        normalized.push(n);
        display.push(d);
        addressed.push(a);
    }
    let automaton = AhoCorasickBuilder::new()
        .match_kind(MatchKind::LeftmostLongest)
        .ascii_case_insensitive(true)
        .build(&normalized)
        .expect("static pattern");
    Injection {
        automaton,
        normalized,
        display,
        addressed,
    }
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexicon_has_enough_distinct_phrases() {
        assert!(PHRASES.len() >= 40, "{}", PHRASES.len());
        let inj = &*INJECTION;
        assert!(inj.normalized.len() > PHRASES.len());
        assert_eq!(inj.normalized.len(), inj.display.len());
        assert!(inj.normalized.iter().all(|p| !p.is_empty()));
    }

    #[test]
    fn variants_are_expanded() {
        for p in [
            "ignore previous instructions",
            "ignore all prior rules",
            "disregard the above",
            "<|imstart|>",
        ] {
            assert!(INJECTION.normalized.iter().any(|n| n == p), "{p}");
        }
    }
}
