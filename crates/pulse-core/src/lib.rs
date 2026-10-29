//! Pulse core: model types, configuration, discovery, parsing and text utilities.

pub mod asset;
pub mod config;
pub mod discover;
pub mod model;
pub mod parse;
pub mod placement;
pub mod refs;
pub mod rule;
pub mod text;

pub use asset::Asset;
pub use config::{Config, ConfigError, DEFAULT_TOML};
pub use discover::{Classifier, DiscoverError, DiscoveredFile, Discovery, discover};
pub use model::{
    AssetKind, Category, Finding, KindSet, LoadMode, Location, Origin, ParseEnumError, Rating,
    RuleMeta, Severity, Span,
};
pub use parse::{JsonDoc, MarkdownView, McpView, SettingsView, ToolsView};
pub use placement::{Misplaced, NearMiss};
pub use refs::ImportGraph;
pub use rule::{OptionsStore, ProjectRule, Rule, RuleCtx, RuleOptions};
pub use text::LineIndex;
