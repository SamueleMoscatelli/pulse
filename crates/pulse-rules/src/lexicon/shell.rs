//! AI-SEC-008/009 word lists: downloaders, interpreters and tool-input sources.

use std::sync::LazyLock;

use aho_corasick::{AhoCorasick, AhoCorasickBuilder};

/// Programs (lowercase) that download remote content.
pub const FETCHERS: &[&str] = &[
    "curl",
    "wget",
    "iwr",
    "irm",
    "invoke-webrequest",
    "invoke-restmethod",
];

/// Programs (lowercase) that execute code read from stdin or an argument.
pub const SHELLS: &[&str] = &[
    "sh",
    "bash",
    "zsh",
    "dash",
    "ksh",
    "fish",
    "python",
    "python3",
    "node",
    "perl",
    "ruby",
    "iex",
    "invoke-expression",
    "pwsh",
    "powershell",
];

/// Builtins that execute their argument as code.
pub const EVALUATORS: &[&str] = &["eval", "source", ".", "iex", "invoke-expression"];

/// Command prefixes skipped when looking for the program (`sudo -E bash`).
pub const WRAPPERS: &[&str] = &["sudo", "env", "command", "exec", "nohup", "time", "doas"];

/// Environment variables that carry tool input into a hook.
pub const TOOL_INPUT_VARS: &[&str] = &["CLAUDE_FILE_PATHS", "CLAUDE_TOOL_INPUT"];

/// Text marking a command substitution or stage as reading tool input (`jq .tool_input.x`).
pub const TOOL_INPUT_MARKER: &str = "tool_input";

/// Prefilter for AI-SEC-008: a downloader or a base64 decoder somewhere in the text.
#[allow(clippy::expect_used)] // static pattern (rust.md)
pub static RCE_HINTS: LazyLock<AhoCorasick> = LazyLock::new(|| {
    AhoCorasickBuilder::new()
        .ascii_case_insensitive(true)
        .build(FETCHERS.iter().copied().chain(["base64"]))
        .expect("static pattern")
});

/// Prefilter for AI-SEC-009: any source of tool input.
#[allow(clippy::expect_used)] // static pattern (rust.md)
pub static TAINT_HINTS: LazyLock<AhoCorasick> = LazyLock::new(|| {
    AhoCorasick::new(TOOL_INPUT_VARS.iter().copied().chain([TOOL_INPUT_MARKER]))
        .expect("static pattern")
});
