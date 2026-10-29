//! `pulse.toml`: typed configuration, loading and validation.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use serde::de::Error as _;
use serde::{Deserialize, Deserializer};

use crate::model::{AssetKind, LoadMode, Rating, Severity};
use crate::rule::RuleOptions;
use crate::text::LineIndex;

/// Config file name looked up in the scan root.
pub const CONFIG_FILE: &str = "pulse.toml";

/// The commented default file written by `pulse init`; parsing it yields `Config::default()`.
pub const DEFAULT_TOML: &str = include_str!("default_pulse.toml");

/// Error loading or validating `pulse.toml`.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("cannot read {}: {source}", path.display())]
    Io { path: PathBuf, source: io::Error },
    #[error("{}:{line}:{col}: {message}", path.display())]
    Parse {
        path: PathBuf,
        line: u32,
        col: u32,
        message: String,
    },
    #[error("{}: {message}", path.display())]
    Invalid { path: PathBuf, message: String },
}

/// The whole `pulse.toml`. Every key is optional.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub scan: Scan,
    pub context: Context,
    pub budgets: Budgets,
    pub gate: Gate,
    /// Per-rule settings keyed by rule ID; IDs are validated against the registry by the engine.
    pub rules: BTreeMap<String, RuleSetting>,
    pub ignore: Vec<Ignore>,
    pub custom_rules: Vec<CustomRule>,
}

/// `[scan]`: what to walk.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Scan {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    /// Globs of folders that store assets for distribution: their assets load OnDemand.
    pub library: Vec<String>,
    pub max_file_bytes: u64,
    pub follow_symlinks: bool,
    pub custom: Vec<CustomAssetDecl>,
}

impl Default for Scan {
    fn default() -> Self {
        Self {
            include: Vec::new(),
            exclude: Vec::new(),
            library: Vec::new(),
            max_file_bytes: 1_048_576,
            follow_symlinks: false,
            custom: Vec::new(),
        }
    }
}

/// `[[scan.custom]]`: an extra asset kind for a custom path.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CustomAssetDecl {
    pub glob: String,
    pub kind: AssetKind,
    /// `None` lets discovery use the kind's default load mode.
    #[serde(default)]
    pub load: Option<LoadMode>,
}

/// `[context]`: token accounting.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Context {
    pub window: u64,
    pub calibration: f64,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            window: 200_000,
            calibration: 1.0,
        }
    }
}

/// `[budgets]`: token budgets per kind, plus count budgets.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Budgets {
    pub instructions: u64,
    pub scoped_instructions: u64,
    pub agent: u64,
    pub prompt: u64,
    pub skill: u64,
    pub tools: u64,
    pub custom: u64,
    pub always_on: u64,
    pub description: u64,
    pub mcp_servers: u64,
    pub tools_count: u64,
}

impl Default for Budgets {
    fn default() -> Self {
        Self {
            instructions: 3000,
            scoped_instructions: 1500,
            agent: 2500,
            prompt: 2000,
            skill: 5000,
            tools: 3000,
            custom: 3000,
            always_on: 8000,
            description: 150,
            mcp_servers: 8,
            tools_count: 40,
        }
    }
}

impl Budgets {
    fn entries(&self) -> [(&'static str, u64); 11] {
        [
            ("instructions", self.instructions),
            ("scoped_instructions", self.scoped_instructions),
            ("agent", self.agent),
            ("prompt", self.prompt),
            ("skill", self.skill),
            ("tools", self.tools),
            ("custom", self.custom),
            ("always_on", self.always_on),
            ("description", self.description),
            ("mcp_servers", self.mcp_servers),
            ("tools_count", self.tools_count),
        ]
    }
}

/// `[gate]`: Quality Gate conditions.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Gate {
    /// `None` (`"none"` in the file) disables the severity condition.
    #[serde(deserialize_with = "de_fail_on")]
    pub fail_on_severity: Option<Severity>,
    pub max_context_debt: f64,
    /// 0 disables the condition.
    pub max_always_on_tokens: u64,
    pub min_ratings: MinRatings,
    pub new_code_only: bool,
}

impl Default for Gate {
    fn default() -> Self {
        Self {
            fail_on_severity: Some(Severity::High),
            max_context_debt: 0.10,
            max_always_on_tokens: 0,
            min_ratings: MinRatings::default(),
            new_code_only: false,
        }
    }
}

/// `gate.min_ratings`: the worst acceptable rating per dimension.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MinRatings {
    pub security: Option<Rating>,
    pub reliability: Option<Rating>,
    pub clarity: Option<Rating>,
    pub efficiency: Option<Rating>,
}

/// A `[rules]` entry: `"off"`, a severity override, or a table of options.
#[derive(Debug, Clone, PartialEq)]
pub enum RuleSetting {
    Off,
    Severity(Severity),
    Options(RuleTable),
}

/// A `[rules.<ID>]` table: optional `severity`/`enabled`, the rest are rule options.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RuleTable {
    pub severity: Option<Severity>,
    pub enabled: Option<bool>,
    pub options: RuleOptions,
}

impl<'de> Deserialize<'de> for RuleSetting {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        match toml::Value::deserialize(d)? {
            toml::Value::String(s) if s.eq_ignore_ascii_case("off") => Ok(Self::Off),
            toml::Value::String(s) => s.parse().map(Self::Severity).map_err(D::Error::custom),
            toml::Value::Table(mut table) => {
                let severity = match table.remove("severity") {
                    None => None,
                    Some(toml::Value::String(s)) => Some(s.parse().map_err(D::Error::custom)?),
                    Some(_) => return Err(D::Error::custom("`severity` must be a string")),
                };
                let enabled = match table.remove("enabled") {
                    None => None,
                    Some(toml::Value::Boolean(b)) => Some(b),
                    Some(_) => return Err(D::Error::custom("`enabled` must be a boolean")),
                };
                Ok(Self::Options(RuleTable {
                    severity,
                    enabled,
                    options: RuleOptions(table),
                }))
            }
            other => Err(D::Error::custom(format!(
                "expected \"off\", a severity or a table, found {}",
                other.type_str()
            ))),
        }
    }
}

/// `[[ignore]]`: suppress findings by path glob.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ignore {
    pub path: String,
    /// Empty means every rule.
    #[serde(default)]
    pub rules: Vec<String>,
    #[serde(default)]
    pub reason: Option<String>,
}

/// `[[custom_rules]]`: a declarative rule with exactly one matcher (`pattern`, `phrases`
/// or `max_tokens`; checked by validation, the regex is compiled by `pulse-rules`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CustomRule {
    pub id: String,
    pub name: String,
    pub severity: Severity,
    /// Empty means every kind.
    #[serde(default)]
    pub kinds: Vec<AssetKind>,
    /// Regex matched on the asset source.
    #[serde(default)]
    pub pattern: Option<String>,
    /// Phrases matched on normalized text.
    #[serde(default)]
    pub phrases: Option<Vec<String>>,
    /// Fires when the asset has more tokens than this.
    #[serde(default)]
    pub max_tokens: Option<u32>,
    pub message: String,
    #[serde(default)]
    pub remediation: Option<String>,
}

fn de_fail_on<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Severity>, D::Error> {
    let s = String::deserialize(d)?;
    if s.eq_ignore_ascii_case("none") {
        return Ok(None);
    }
    s.parse().map(Some).map_err(D::Error::custom)
}

impl Config {
    /// Loads `explicit` if given (it must exist), else `<root>/pulse.toml`, else the defaults.
    pub fn load(root: &Path, explicit: Option<&Path>) -> Result<Self, ConfigError> {
        let path = explicit.map_or_else(|| root.join(CONFIG_FILE), Path::to_path_buf);
        let source = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) if e.kind() == io::ErrorKind::NotFound && explicit.is_none() => {
                return Ok(Self::default());
            }
            Err(source) => return Err(ConfigError::Io { path, source }),
        };
        Self::parse(&source, &path)
    }

    /// Parses and validates `source`; `path` is only used in error messages.
    pub fn parse(source: &str, path: &Path) -> Result<Self, ConfigError> {
        let config: Self = toml::from_str(source).map_err(|e| {
            let (line, col) = e.span().map_or((1, 1), |span| {
                let byte = u32::try_from(span.start).unwrap_or(u32::MAX);
                LineIndex::new(source).location(source, byte)
            });
            ConfigError::Parse {
                path: path.to_path_buf(),
                line,
                col,
                message: with_suggestion(e.message()),
            }
        })?;
        config.validate().map_err(|message| ConfigError::Invalid {
            path: path.to_path_buf(),
            message,
        })?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), String> {
        if !(0.0..=1.0).contains(&self.gate.max_context_debt) {
            return Err(format!(
                "gate.max_context_debt must be between 0 and 1, got {}",
                self.gate.max_context_debt
            ));
        }
        if self.context.window == 0 {
            return Err("context.window must be greater than 0".to_owned());
        }
        if !(self.context.calibration.is_finite() && self.context.calibration > 0.0) {
            return Err(format!(
                "context.calibration must be greater than 0, got {}",
                self.context.calibration
            ));
        }
        if let Some((name, _)) = self.budgets.entries().into_iter().find(|&(_, v)| v == 0) {
            return Err(format!("budgets.{name} must be greater than 0"));
        }
        if let Some(rule) = self.custom_rules.iter().find(|r| !is_custom_rule_id(&r.id)) {
            return Err(format!(
                "custom rule ID `{}` must match X-<ORG>-<NNN> (e.g. X-ACME-001)",
                rule.id
            ));
        }
        for (i, rule) in self.custom_rules.iter().enumerate() {
            if self.custom_rules[..i].iter().any(|r| r.id == rule.id) {
                return Err(format!("duplicate custom rule ID `{}`", rule.id));
            }
            let matchers = usize::from(rule.pattern.is_some())
                + usize::from(rule.phrases.is_some())
                + usize::from(rule.max_tokens.is_some());
            if matchers != 1 {
                return Err(format!(
                    "custom rule `{}` needs exactly one of `pattern`, `phrases`, `max_tokens`",
                    rule.id
                ));
            }
            if rule.pattern.as_deref() == Some("") {
                return Err(format!("custom rule `{}`: `pattern` is empty", rule.id));
            }
            if rule
                .phrases
                .as_ref()
                .is_some_and(|p| p.is_empty() || p.iter().any(|p| p.trim().is_empty()))
            {
                return Err(format!(
                    "custom rule `{}`: `phrases` must be non-empty strings",
                    rule.id
                ));
            }
            if rule.max_tokens == Some(0) {
                return Err(format!(
                    "custom rule `{}`: `max_tokens` must be greater than 0",
                    rule.id
                ));
            }
        }
        Ok(())
    }
}

/// Whether `id` matches `^X-[A-Z0-9]+-\d{3}$`.
pub fn is_custom_rule_id(id: &str) -> bool {
    let Some(rest) = id.strip_prefix("X-") else {
        return false;
    };
    let Some((org, num)) = rest.rsplit_once('-') else {
        return false;
    };
    !org.is_empty()
        && org
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        && num.len() == 3
        && num.bytes().all(|b| b.is_ascii_digit())
}

/// The candidate closest to `word` by edit distance, if close enough to be a likely typo.
pub fn did_you_mean<'a>(
    word: &str,
    candidates: impl IntoIterator<Item = &'a str>,
) -> Option<&'a str> {
    let max = (word.chars().count() / 3).max(1);
    candidates
        .into_iter()
        .map(|c| (levenshtein(word, c), c))
        .filter(|&(d, _)| d <= max)
        .min()
        .map(|(_, c)| c)
}

fn levenshtein(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut diag = row[0];
        row[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let next = (diag + usize::from(ca != cb))
                .min(row[j] + 1)
                .min(row[j + 1] + 1);
            diag = row[j + 1];
            row[j + 1] = next;
        }
    }
    row[b.len()]
}

/// Appends a "did you mean" hint to serde's "unknown field `x`, expected one of `a`, `b`" message.
fn with_suggestion(message: &str) -> String {
    let message = message.trim_end();
    if !message.starts_with("unknown field") {
        return message.to_owned();
    }
    // Backticked names: the unknown field first, then the expected ones.
    let mut names = message.split('`').skip(1).step_by(2);
    let Some(unknown) = names.next() else {
        return message.to_owned();
    };
    match did_you_mean(unknown, names) {
        Some(s) => format!("{message} (did you mean `{s}`?)"),
        None => message.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> Result<Config, ConfigError> {
        Config::parse(src, Path::new("pulse.toml"))
    }

    #[test]
    fn default_toml_parses_to_default_config() {
        assert_eq!(parse(DEFAULT_TOML).unwrap(), Config::default());
    }

    #[test]
    fn empty_file_is_default() {
        assert_eq!(parse("").unwrap(), Config::default());
    }

    #[test]
    fn unknown_key_errors_with_location_and_suggestion() {
        let err = parse("[scan]\nexclude = []\nexlude = [\"x\"]\n").unwrap_err();
        let msg = err.to_string();
        assert!(
            matches!(
                err,
                ConfigError::Parse {
                    line: 3,
                    col: 1,
                    ..
                }
            ),
            "{msg}"
        );
        assert!(msg.contains("unknown field `exlude`"), "{msg}");
        assert!(msg.contains("did you mean `exclude`?"), "{msg}");
    }

    #[test]
    fn unknown_top_level_key_errors() {
        let msg = parse("[gates]\n").unwrap_err().to_string();
        assert!(msg.contains("did you mean `gate`?"), "{msg}");
    }

    #[test]
    fn rule_settings_parse_off_severity_and_options() {
        let cfg = parse(
            r#"
[rules]
"AI-COG-003" = "off"
"AI-COG-004" = "Medium"

[rules.AI-SEC-010]
severity = "critical"
allowed_domains = ["github.com"]
"#,
        )
        .unwrap();
        assert_eq!(cfg.rules["AI-COG-003"], RuleSetting::Off);
        assert_eq!(
            cfg.rules["AI-COG-004"],
            RuleSetting::Severity(Severity::Medium)
        );
        let RuleSetting::Options(t) = &cfg.rules["AI-SEC-010"] else {
            panic!("expected options");
        };
        assert_eq!(t.severity, Some(Severity::Critical));
        assert_eq!(t.enabled, None);
        assert!(t.options.get("allowed_domains").is_some());
        assert!(t.options.get("severity").is_none());
    }

    #[test]
    fn bad_rule_severity_errors_at_value() {
        let err = parse("[rules]\n\"AI-COG-004\" = \"severe\"\n").unwrap_err();
        let msg = err.to_string();
        assert!(matches!(err, ConfigError::Parse { line: 2, .. }), "{msg}");
        assert!(msg.contains("unknown severity `severe`"), "{msg}");
    }

    #[test]
    fn gate_fail_on_none_disables() {
        let cfg =
            parse("[gate]\nfail_on_severity = \"none\"\nmin_ratings = { security = \"A\" }\n")
                .unwrap();
        assert_eq!(cfg.gate.fail_on_severity, None);
        assert_eq!(cfg.gate.min_ratings.security, Some(Rating::A));
    }

    #[test]
    fn scan_custom_and_ignore_and_custom_rules_parse() {
        let cfg = parse(
            r#"
[[scan.custom]]
glob = "prompts/**/*.txt"
kind = "prompt"
load = "on_demand"

[[ignore]]
path = "docs/attacks/**"
rules = ["AI-SEC-002"]

[[custom_rules]]
id = "X-ORG-001"
name = "No internal hostnames"
severity = "high"
kinds = ["instructions", "skill"]
pattern = 'internal\.corp'
message = "Internal hostname"
"#,
        )
        .unwrap();
        assert_eq!(cfg.scan.custom[0].kind, AssetKind::Prompt);
        assert_eq!(cfg.scan.custom[0].load, Some(LoadMode::OnDemand));
        assert_eq!(cfg.ignore[0].rules, ["AI-SEC-002"]);
        assert_eq!(
            cfg.custom_rules[0].kinds,
            [AssetKind::Instructions, AssetKind::Skill]
        );
    }

    #[test]
    fn validation_rejects_out_of_range_values() {
        for (src, needle) in [
            ("[gate]\nmax_context_debt = 1.5\n", "max_context_debt"),
            ("[budgets]\nskill = 0\n", "budgets.skill"),
            ("[context]\ncalibration = 0.0\n", "calibration"),
            (
                "[[custom_rules]]\nid = \"ORG-1\"\nname = \"n\"\nseverity = \"low\"\npattern = \"x\"\nmessage = \"m\"\n",
                "custom rule ID `ORG-1`",
            ),
            (
                "[[custom_rules]]\nid = \"X-A-001\"\nname = \"n\"\nseverity = \"low\"\nmessage = \"m\"\n",
                "exactly one of",
            ),
            (
                "[[custom_rules]]\nid = \"X-A-001\"\nname = \"n\"\nseverity = \"low\"\npattern = \"x\"\nmax_tokens = 5\nmessage = \"m\"\n",
                "exactly one of",
            ),
            (
                "[[custom_rules]]\nid = \"X-A-001\"\nname = \"n\"\nseverity = \"low\"\nphrases = []\nmessage = \"m\"\n",
                "`phrases` must be",
            ),
            (
                "[[custom_rules]]\nid = \"X-A-001\"\nname = \"n\"\nseverity = \"low\"\nmax_tokens = 0\nmessage = \"m\"\n",
                "`max_tokens` must be",
            ),
            (
                "[[custom_rules]]\nid = \"X-A-001\"\nname = \"n\"\nseverity = \"low\"\nmax_tokens = 1\nmessage = \"m\"\n[[custom_rules]]\nid = \"X-A-001\"\nname = \"n\"\nseverity = \"low\"\nmax_tokens = 2\nmessage = \"m\"\n",
                "duplicate custom rule ID",
            ),
        ] {
            let err = parse(src).unwrap_err();
            assert!(matches!(err, ConfigError::Invalid { .. }), "{err}");
            assert!(err.to_string().contains(needle), "{err}");
        }
    }

    #[test]
    fn custom_rule_id_format() {
        assert!(is_custom_rule_id("X-ORG-001"));
        assert!(is_custom_rule_id("X-A1-999"));
        for bad in [
            "X-org-001",
            "X--001",
            "X-ORG-01",
            "AI-SEC-001",
            "X-ORG-0011",
        ] {
            assert!(!is_custom_rule_id(bad), "{bad}");
        }
    }

    #[test]
    fn load_missing_file_is_default_unless_explicit() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(Config::load(dir.path(), None).unwrap(), Config::default());
        let missing = dir.path().join("other.toml");
        assert!(matches!(
            Config::load(dir.path(), Some(&missing)),
            Err(ConfigError::Io { .. })
        ));
    }

    #[test]
    fn load_reads_root_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(CONFIG_FILE), "[context]\nwindow = 1000\n").unwrap();
        assert_eq!(Config::load(dir.path(), None).unwrap().context.window, 1000);
    }
}
