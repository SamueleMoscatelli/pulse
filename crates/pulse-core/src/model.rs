//! Core vocabulary: severities, categories, asset kinds, locations and findings.

use std::borrow::Cow;
use std::fmt;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::asset::Asset;

/// Error for parsing an enum from its config/CLI string form.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown {what} `{value}` (expected one of: {expected})")]
pub struct ParseEnumError {
    pub what: &'static str,
    pub value: String,
    pub expected: &'static str,
}

/// Generates `as_str`, `ALL`, `Display` and case-insensitive `FromStr` for a fieldless enum.
macro_rules! string_enum {
    ($ty:ident, $what:literal, { $($variant:ident => $s:literal),+ $(,)? }) => {
        impl $ty {
            /// Every variant, in declaration order.
            pub const ALL: &'static [$ty] = &[$($ty::$variant),+];

            /// The canonical string form, as used in config and output.
            pub const fn as_str(self) -> &'static str {
                match self { $($ty::$variant => $s),+ }
            }
        }

        impl fmt::Display for $ty {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl FromStr for $ty {
            type Err = ParseEnumError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                $(if s.eq_ignore_ascii_case($s) { return Ok($ty::$variant); })+
                Err(ParseEnumError {
                    what: $what,
                    value: s.to_owned(),
                    expected: concat!($($s, " "),+),
                })
            }
        }
    };
}

/// How bad a finding is. Ordered `Info < Low < Medium < High < Critical`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

string_enum!(Severity, "severity", {
    Info => "info", Low => "low", Medium => "medium", High => "high", Critical => "critical",
});

/// Rule family; determines the rule ID prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Security,
    Structure,
    Efficiency,
    Cognitive,
}

string_enum!(Category, "category", {
    Security => "security", Structure => "structure", Efficiency => "efficiency", Cognitive => "cognitive",
});

impl Category {
    /// The rule ID prefix (`AI-<prefix>-NNN`).
    pub const fn prefix(self) -> &'static str {
        match self {
            Category::Security => "SEC",
            Category::Structure => "STR",
            Category::Efficiency => "EFF",
            Category::Cognitive => "COG",
        }
    }
}

/// What kind of AI asset a file is (see the discovery table).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    Instructions,
    ScopedInstructions,
    Agent,
    Prompt,
    Skill,
    Tools,
    McpConfig,
    Settings,
    Hooks,
    Custom,
}

string_enum!(AssetKind, "asset kind", {
    Instructions => "instructions",
    ScopedInstructions => "scoped_instructions",
    Agent => "agent",
    Prompt => "prompt",
    Skill => "skill",
    Tools => "tools",
    McpConfig => "mcp_config",
    Settings => "settings",
    Hooks => "hooks",
    Custom => "custom",
});

impl AssetKind {
    /// This kind's bit in a [`KindSet`].
    pub const fn bit(self) -> u16 {
        1 << self as u16
    }

    /// Load mode from the discovery table, before frontmatter/nesting refinements.
    /// Skill and Agent are `Always`: only their description counts as always-on.
    pub const fn default_load(self) -> LoadMode {
        match self {
            Self::ScopedInstructions => LoadMode::Conditional,
            Self::Prompt | Self::Custom => LoadMode::OnDemand,
            Self::Instructions
            | Self::Agent
            | Self::Skill
            | Self::Tools
            | Self::McpConfig
            | Self::Settings
            | Self::Hooks => LoadMode::Always,
        }
    }
}

/// Which discovery row found an asset (spec 02-assets → Discovery table).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Origin {
    /// Rows 1–9: a file an assistant loads from where it is.
    #[default]
    Platform,
    /// Row 10 (ADR-041): matched by suffix only, outside the folders assistants load.
    LibraryRow,
    /// Rows 1–10 under a `scan.library` glob: declared as a distribution folder.
    Library,
    /// Inside a Kiro power (ADR-054): a distribution folder like [`Origin::Library`].
    Power,
    /// `[[scan.custom]]` or `scan.include`.
    Custom,
}

/// When an asset is loaded into the model's context; drives context-load metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoadMode {
    Always,
    Conditional,
    OnDemand,
}

string_enum!(LoadMode, "load mode", {
    Always => "always", Conditional => "conditional", OnDemand => "on_demand",
});

/// Sonar-style rating, `A` (best) to `E` (worst). Ordered `A < B < … < E`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Rating {
    A,
    B,
    C,
    D,
    E,
}

string_enum!(Rating, "rating", { A => "A", B => "B", C => "C", D => "D", E => "E" });

/// A set of [`AssetKind`]s, stored as a `u16` bitset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct KindSet(u16);

impl KindSet {
    pub const EMPTY: Self = Self(0);
    pub const INSTRUCTIONS: Self = Self::of(AssetKind::Instructions);
    pub const SCOPED_INSTRUCTIONS: Self = Self::of(AssetKind::ScopedInstructions);
    pub const AGENT: Self = Self::of(AssetKind::Agent);
    pub const PROMPT: Self = Self::of(AssetKind::Prompt);
    pub const SKILL: Self = Self::of(AssetKind::Skill);
    pub const TOOLS: Self = Self::of(AssetKind::Tools);
    pub const MCP_CONFIG: Self = Self::of(AssetKind::McpConfig);
    pub const SETTINGS: Self = Self::of(AssetKind::Settings);
    pub const HOOKS: Self = Self::of(AssetKind::Hooks);
    pub const CUSTOM: Self = Self::of(AssetKind::Custom);
    /// Every kind.
    pub const ALL: Self = Self((1 << AssetKind::ALL.len()) - 1);
    /// Kinds whose source is Markdown (with or without frontmatter).
    pub const MARKDOWN: Self = Self::INSTRUCTIONS
        .union(Self::SCOPED_INSTRUCTIONS)
        .union(Self::AGENT)
        .union(Self::PROMPT)
        .union(Self::SKILL);

    /// The set containing only `kind`.
    pub const fn of(kind: AssetKind) -> Self {
        Self(kind.bit())
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    pub const fn with(self, kind: AssetKind) -> Self {
        Self(self.0 | kind.bit())
    }

    pub const fn without(self, kind: AssetKind) -> Self {
        Self(self.0 & !kind.bit())
    }

    pub const fn contains(self, kind: AssetKind) -> bool {
        self.0 & kind.bit() != 0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub const fn len(self) -> u32 {
        self.0.count_ones()
    }

    /// Kinds in the set, in declaration order.
    pub fn iter(self) -> impl Iterator<Item = AssetKind> {
        AssetKind::ALL
            .iter()
            .copied()
            .filter(move |k| self.contains(*k))
    }
}

impl std::ops::BitOr for KindSet {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        self.union(rhs)
    }
}

impl std::ops::BitAnd for KindSet {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        self.intersection(rhs)
    }
}

impl FromIterator<AssetKind> for KindSet {
    fn from_iter<I: IntoIterator<Item = AssetKind>>(iter: I) -> Self {
        iter.into_iter().fold(Self::EMPTY, Self::with)
    }
}

/// Byte range `[start, end)` into an asset's source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub const fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }
}

/// A resolved position in a file; lines and columns are 1-based, columns count chars.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Location {
    pub path: Arc<Path>,
    pub line: u32,
    pub col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

/// Static description of a rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleMeta {
    pub id: &'static str,
    pub name: &'static str,
    pub category: Category,
    pub default_severity: Severity,
    pub kinds: KindSet,
    pub summary: &'static str,
    pub remediation: &'static str,
    /// `false` for critical SEC rules (ADR-006).
    pub inline_suppressible: bool,
}

/// One problem reported by a rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub rule_id: &'static str,
    /// Effective severity, after config overrides.
    pub severity: Severity,
    pub location: Location,
    pub message: String,
    /// The rule's default fix, or a per-finding one (see [`Finding::with_fix`]).
    pub remediation: Option<Cow<'static, str>>,
    /// Estimated wasted tokens; `> 0` only for efficiency findings (feeds context debt).
    pub token_waste: u32,
    /// Stable identity for baselines; set by the engine.
    pub fingerprint: u64,
}

impl Finding {
    /// A finding at `span` in `asset`, with the rule's default severity and remediation.
    pub fn new(meta: &RuleMeta, asset: &Asset, span: Span, message: impl Into<String>) -> Self {
        Self {
            rule_id: meta.id,
            severity: meta.default_severity,
            location: asset.span_to_location(span),
            message: message.into(),
            remediation: Some(Cow::Borrowed(meta.remediation)),
            token_waste: 0,
            fingerprint: 0,
        }
    }

    /// Replaces the rule's default fix with one specific to this finding.
    #[must_use]
    pub fn with_fix(mut self, fix: impl Into<Cow<'static, str>>) -> Self {
        self.remediation = Some(fix.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_ordering_is_ascending() {
        let mut v = vec![
            Severity::Critical,
            Severity::Info,
            Severity::High,
            Severity::Low,
        ];
        v.sort();
        assert_eq!(
            v,
            [
                Severity::Info,
                Severity::Low,
                Severity::High,
                Severity::Critical
            ]
        );
        assert!(Severity::Medium > Severity::Low);
    }

    #[test]
    fn severity_parsing_roundtrips() {
        for &s in Severity::ALL {
            assert_eq!(s.to_string().parse::<Severity>(), Ok(s));
        }
        assert_eq!("HIGH".parse::<Severity>(), Ok(Severity::High));
        let err = "severe".parse::<Severity>().unwrap_err();
        assert!(err.to_string().contains("severe"));
    }

    #[test]
    fn severity_serde_is_lowercase() {
        assert_eq!(
            serde_json::to_string(&Severity::Critical).unwrap(),
            "\"critical\""
        );
        let s: Severity = serde_json::from_str("\"medium\"").unwrap();
        assert_eq!(s, Severity::Medium);
    }

    #[test]
    fn asset_kind_parses_snake_case() {
        assert_eq!("mcp_config".parse(), Ok(AssetKind::McpConfig));
        assert_eq!(
            "scoped_instructions".parse(),
            Ok(AssetKind::ScopedInstructions)
        );
        assert!("mcpconfig".parse::<AssetKind>().is_err());
        for &k in AssetKind::ALL {
            assert_eq!(k.as_str().parse(), Ok(k));
            assert_eq!(serde_json::to_string(&k).unwrap(), format!("\"{k}\""));
        }
    }

    #[test]
    fn category_prefixes() {
        let p: Vec<_> = Category::ALL.iter().map(|c| c.prefix()).collect();
        assert_eq!(p, ["SEC", "STR", "EFF", "COG"]);
    }

    #[test]
    fn kindset_ops() {
        let s = KindSet::MCP_CONFIG | KindSet::HOOKS;
        assert!(s.contains(AssetKind::McpConfig) && s.contains(AssetKind::Hooks));
        assert!(!s.contains(AssetKind::Agent));
        assert_eq!(s.len(), 2);
        assert_eq!(s & KindSet::HOOKS, KindSet::HOOKS);
        assert!(
            s.without(AssetKind::Hooks)
                .without(AssetKind::McpConfig)
                .is_empty()
        );
        assert_eq!(KindSet::EMPTY.with(AssetKind::Skill), KindSet::SKILL);
        assert_eq!(
            s.iter().collect::<Vec<_>>(),
            [AssetKind::McpConfig, AssetKind::Hooks]
        );
        assert_eq!(s.iter().collect::<KindSet>(), s);
    }

    #[test]
    fn kindset_all_and_markdown() {
        assert_eq!(KindSet::ALL.len() as usize, AssetKind::ALL.len());
        assert!(AssetKind::ALL.iter().all(|&k| KindSet::ALL.contains(k)));
        assert_eq!(KindSet::MARKDOWN.len(), 5);
        assert!(KindSet::MARKDOWN.contains(AssetKind::Skill));
        assert!(!KindSet::MARKDOWN.contains(AssetKind::McpConfig));
        assert!(!KindSet::MARKDOWN.contains(AssetKind::Custom));
    }
}
