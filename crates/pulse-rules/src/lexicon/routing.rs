//! Routing-description vocabulary (AI-COG-006). Comparisons ignore ASCII case.

/// Word prefixes that are a trigger cue on their own (`triggers`, `invoked`, `proactively`).
pub const CUE_PREFIXES: &[&str] = &["trigger", "invok", "proactive"];

/// `use` forms that are a cue when a [`USE_SCOPES`] word follows within [`USE_WINDOW`]
/// words (`Use when …`, `Use this agent for …`, `Should be used whenever …`).
pub const USE_FORMS: &[&str] = &["use", "used"];
pub const USE_SCOPES: &[&str] = &["when", "whenever", "for", "if"];
pub const USE_WINDOW: usize = 4;

/// `when the user …`: a [`WHEN`] word followed by `user` (optionally after an article).
pub const WHEN: &[&str] = &["when", "whenever", "if"];
pub const ARTICLES: &[&str] = &["the", "a"];

/// First-person words (case-sensitive: a lowercase `i` is usually a variable).
pub const FIRST_PERSON: &[&str] = &[
    "I", "I'm", "I’m", "I'll", "I’ll", "I've", "I’ve", "I'd", "I’d",
];
