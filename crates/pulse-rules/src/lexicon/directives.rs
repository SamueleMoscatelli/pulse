//! Directive vocabulary (AI-COG-001, AI-COG-005). Words are lowercase; comparisons ignore
//! ASCII case. Multi-word entries are written as lemmas (no plural `s`).

/// Words that open a positive directive anywhere in a sentence.
pub const POSITIVE: &[&str] = &["always", "must", "should", "use", "prefer"];

/// Words that open a negative directive anywhere in a sentence.
pub const NEGATIVE: &[&str] = &[
    "never",
    "avoid",
    "don't",
    "don’t",
    "dont",
    "mustn't",
    "mustn’t",
    "shouldn't",
    "shouldn’t",
];

/// Markers that open a directive only as a sentence's first word (`Do …`, `No …`).
pub const POSITIVE_FIRST: &[&str] = &["do"];
pub const NEGATIVE_FIRST: &[&str] = &["no"];

/// Words that may continue a marker run (`must not`, `should always`, `do not use`).
pub const MODALS: &[&str] = &["always", "must", "should", "do", "please", "not"];

/// Negators inside a marker run.
pub const NEGATORS: &[&str] = &[
    "never",
    "not",
    "avoid",
    "don't",
    "don’t",
    "dont",
    "mustn't",
    "mustn’t",
    "shouldn't",
    "shouldn’t",
];

/// Verbs folded into `use` (`avoid X` is the negation of `use X`).
pub const USE_VERBS: &[&str] = &["use", "using", "prefer", "avoid"];

/// Words that introduce a scope: the words after them are the directive's context.
pub const QUALIFIERS: &[&str] = &[
    "when", "if", "for", "in", "except", "unless", "only", "while", "inside", "within", "during",
];

/// Words that end a directive's object (`prefer X over Y`, `use X instead of Y`).
pub const TERMINATORS: &[&str] = &[
    "over", "instead", "rather", "than", "but", "because", "since",
];

/// Words dropped from objects and contexts.
pub const STOP: &[&str] = &[
    "a", "an", "the", "your", "our", "my", "their", "its", "it", "this", "that", "these", "those",
    "to", "of", "with", "as", "be", "is", "are", "and", "or", "by", "on", "at", "from", "all",
    "every", "each", "some", "them", "you", "we", "also", "please", "just", "very", "always",
];

/// Heading words that name no scope (`Code style`, `General rules`, `Do and don't`).
pub const GENERIC_HEADINGS: &[&str] = &[
    "rule",
    "guideline",
    "convention",
    "style",
    "code",
    "coding",
    "general",
    "overview",
    "important",
    "instruction",
    "note",
    "standard",
    "best",
    "practice",
    "preference",
    "do",
    "don't",
    "don’t",
    "dont",
    "dos",
    "donts",
    "and",
    "project",
    "basic",
    "requirement",
    "principle",
    "policy",
    "policie",
    "misc",
    "other",
];

/// Choices that cannot both be prescribed (`[rules.AI-COG-001] exclusive` adds more).
pub const EXCLUSIVE_SETS: &[&[&str]] = &[
    &["typescript", "javascript"],
    &["tab", "space"],
    &["npm", "yarn", "pnpm", "bun"],
    &["single quote", "double quote"],
    &["jest", "vitest", "mocha"],
    &["snake_case", "camelcase"],
    &["pip", "poetry", "uv", "pipenv"],
    &["pytest", "unittest"],
    &["eslint", "biome"],
    &["webpack", "vite"],
    &["commonjs", "esm"],
    &["rest", "graphql"],
    &["merge", "rebase"],
    &["lf", "crlf"],
    &["maven", "gradle"],
];

/// Words that offer a positive alternative to a negative directive (AI-COG-003), when no
/// negator directly precedes them (`never use` is not one).
pub const ALTERNATIVES: &[&str] = &["instead", "use", "using", "prefer", "rather", "focus"];

/// Words that may precede a prohibition's opener (`Please never …`, `otherwise do not …`).
pub const LEAD_INS: &[&str] = &[
    "please",
    "also",
    "and",
    "but",
    "so",
    "then",
    "otherwise",
    "just",
    "again",
];

/// Subjects allowed before a modal prohibition (`You must not …`).
pub const SUBJECTS: &[&str] = &["you", "we"];

/// Modals that forbid when negated (`must not`, `should never`).
pub const PROHIBITING_MODALS: &[&str] = &["must", "should", "shall"];

/// Terms that make a prohibition vague (AI-COG-003), besides [`super::style::VAGUE`].
pub const VAGUE_LIMITS: &[&str] = &[
    "unnecessary",
    "unnecessarily",
    "too much",
    "too many",
    "excessive",
    "excessively",
    "verbose",
    "bad",
    "wrong",
];
