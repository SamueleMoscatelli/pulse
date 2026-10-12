//! Pulse core: model types, configuration, discovery, parsing and text utilities.

pub mod asset;
pub mod config;
pub mod model;
pub mod rule;
pub mod text;

pub use asset::Asset;
pub use config::{Config, ConfigError, DEFAULT_TOML};
pub use model::{
    AssetKind, Category, Finding, KindSet, LoadMode, Location, ParseEnumError, Rating, RuleMeta,
    Severity, Span,
};
pub use rule::{ProjectRule, Rule, RuleCtx, RuleOptions};
pub use text::LineIndex;
