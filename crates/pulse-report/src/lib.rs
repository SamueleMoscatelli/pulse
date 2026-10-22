//! Pulse report renderers: terminal, JSON, SARIF, Markdown, GitHub.

mod github;
mod json;
mod markdown;
mod sarif;
mod terminal;

use std::path::Path;

use pulse_core::Finding;

pub use github::{GithubOptions, render as render_github};
pub use json::{JsonOptions, SCHEMA as JSON_SCHEMA, render as render_json};
pub use markdown::{MarkdownOptions, render as render_markdown};
pub use sarif::{SarifOptions, render as render_sarif};
pub use terminal::{TerminalOptions, render as render_terminal};

/// `path` relative to `root`, with `/` separators.
fn rel_path(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Total open findings in `m`.
fn issue_count(m: &pulse_engine::Metrics) -> usize {
    m.issues_by_severity.values().sum()
}

/// A finding's fingerprint as 16 lowercase hex digits.
fn fingerprint(f: &Finding) -> String {
    format!("{:016x}", f.fingerprint)
}
