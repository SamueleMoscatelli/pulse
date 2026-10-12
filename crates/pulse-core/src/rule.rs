//! Rule traits and the context they run with.

use std::collections::HashSet;
use std::path::Path;

use crate::asset::Asset;
use crate::model::{Finding, RuleMeta};

/// A rule that inspects one asset at a time.
pub trait Rule: Sync {
    fn meta(&self) -> &'static RuleMeta;
    fn check(&self, asset: &Asset, ctx: &RuleCtx<'_>, out: &mut Vec<Finding>);
}

/// A rule that needs to see every asset of the project at once.
pub trait ProjectRule: Sync {
    fn meta(&self) -> &'static RuleMeta;
    fn check_project(&self, assets: &[Asset], ctx: &RuleCtx<'_>, out: &mut Vec<Finding>);
}

/// Per-rule options resolved from `pulse.toml` (`[rules.<ID>]` table).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RuleOptions(pub toml::Table);

impl RuleOptions {
    pub fn get(&self, key: &str) -> Option<&toml::Value> {
        self.0.get(key)
    }
}

/// Everything a rule may consult besides the asset itself.
#[derive(Debug, Clone, Copy)]
pub struct RuleCtx<'a> {
    pub options: &'a RuleOptions,
    pub root: &'a Path,
    /// Every walked file, as a scan-root-relative path with `/` separators.
    pub files: &'a HashSet<String>,
}

impl RuleCtx<'_> {
    /// Whether `rel_path` (scan-root-relative, `/` separators) was seen by the walker.
    pub fn file_exists(&self, rel_path: &str) -> bool {
        self.files.contains(rel_path)
    }
}
