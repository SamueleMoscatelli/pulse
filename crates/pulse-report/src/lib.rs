//! Pulse report renderers: terminal, JSON, SARIF, Markdown, GitHub.

mod terminal;

pub use terminal::{TerminalOptions, render as render_terminal};
