//! Pulse engine: discover, parse, run rules, suppress, baseline, score, gate.

mod pipeline;
mod post;

use std::path::PathBuf;
use std::time::Duration;

use pulse_core::{AssetKind, ConfigError, DiscoverError, Finding, LoadMode};

pub use pipeline::scan;

/// Error that aborts a scan (bad assets never do: they become findings).
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Discover(#[from] DiscoverError),
}

/// Narrows a scan; every field empty means "everything".
#[derive(Debug, Clone, Default)]
pub struct ScanOptions {
    /// Scan-root-relative files or directories (`/` separators) whose findings are kept.
    pub paths: Vec<String>,
    /// Run only rules whose ID starts with one of these prefixes.
    pub only_rules: Vec<String>,
    /// Skip rules whose ID starts with one of these prefixes.
    pub skip_rules: Vec<String>,
}

/// Per-asset facts for metrics and reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetMeta {
    pub rel_path: String,
    pub kind: AssetKind,
    /// Effective load mode (after frontmatter promotion).
    pub load: LoadMode,
    pub tokens: u32,
    pub always_tokens: u32,
}

/// Wall-clock time of each scan phase.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Timings {
    pub discover: Duration,
    pub read: Duration,
    pub rules: Duration,
    pub total: Duration,
}

/// Result of a scan. `findings` are sorted by (path, line, col, rule); `assets_meta` by path.
#[derive(Debug, Clone, Default)]
pub struct Report {
    pub findings: Vec<Finding>,
    pub assets_meta: Vec<AssetMeta>,
    /// Entries the walker could not visit, with the reason.
    pub walk_errors: Vec<(PathBuf, String)>,
    pub timings: Timings,
}
