//! Pulse engine: discover, parse, run rules, suppress, baseline, score, gate.

mod baseline;
mod gate;
mod pipeline;
mod post;
mod score;
mod suppress;

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Duration;

use pulse_core::{AssetKind, ConfigError, DiscoverError, Finding, LoadMode};

pub use baseline::{BASELINE_FILE, Baseline, BaselineError};
pub use gate::{Condition, Gate, GateStatus, GateValue};
pub use pipeline::{RuleStatus, rule_statuses, scan};
pub use score::{Metrics, Ratings, category_of};

/// Error that aborts a scan (bad assets never do: they become findings).
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Discover(#[from] DiscoverError),
    #[error(transparent)]
    Baseline(#[from] BaselineError),
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
    /// When set, only these scan-root-relative files are in scope (`--changed-since`).
    pub changed: Option<BTreeSet<String>>,
    /// Accepted findings; the rest are new. `None` means every finding is new.
    pub baseline: Option<Baseline>,
    /// Keep only new findings (`--new-only`); also turns on new-code ratings and gate.
    pub new_only: bool,
}

impl ScanOptions {
    /// Whether the asset at `rel` is in scope: under `paths` and, if set, in `changed`.
    pub fn in_scope(&self, rel: &str) -> bool {
        pipeline::in_paths(rel, &self.paths)
            && self.changed.as_ref().is_none_or(|c| c.contains(rel))
    }
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
    /// Findings dropped by inline or `[[ignore]]` suppressions.
    pub suppressed: usize,
    pub assets_meta: Vec<AssetMeta>,
    /// Entries the walker could not visit, with the reason.
    pub walk_errors: Vec<(PathBuf, String)>,
    /// Metrics over the in-scope assets and every open finding (new or not).
    pub metrics: Metrics,
    /// Metrics over new findings only, when new-code mode is on (`--new-only` or
    /// `gate.new_code_only`); ratings and gate are then computed from these.
    pub new_metrics: Option<Metrics>,
    pub ratings: Ratings,
    pub gate: Gate,
    /// The baseline the findings were compared with.
    pub baseline: Option<Baseline>,
    pub timings: Timings,
}

impl Report {
    /// Whether `f` is not in the baseline (always true without one).
    pub fn is_new(&self, f: &Finding) -> bool {
        self.baseline.as_ref().is_none_or(|b| b.is_new(f))
    }
}
