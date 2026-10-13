//! Rule traits and the context they run with.

use std::any::{Any, TypeId};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, PoisonError, RwLock};

use serde::de::DeserializeOwned;

use crate::asset::Asset;
use crate::config::{CONFIG_FILE, ConfigError};
use crate::model::{Finding, RuleMeta};

/// A rule that inspects one asset at a time.
pub trait Rule: Sync {
    fn meta(&self) -> &'static RuleMeta;
    fn check(&self, asset: &Asset, ctx: &RuleCtx<'_>, out: &mut Vec<Finding>);
    /// Validates this rule's options once per scan, before any check (e.g. by calling
    /// [`RuleCtx::options`] with the rule's options type).
    fn validate(&self, _ctx: &RuleCtx<'_>) -> Result<(), ConfigError> {
        Ok(())
    }
}

/// A rule that needs to see every asset of the project at once.
pub trait ProjectRule: Sync {
    fn meta(&self) -> &'static RuleMeta;
    fn check_project(&self, assets: &[Asset], ctx: &RuleCtx<'_>, out: &mut Vec<Finding>);
    /// Validates this rule's options once per scan; see [`Rule::validate`].
    fn validate(&self, _ctx: &RuleCtx<'_>) -> Result<(), ConfigError> {
        Ok(())
    }
}

/// Per-rule options resolved from `pulse.toml` (`[rules.<ID>]` table).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RuleOptions(pub toml::Table);

impl RuleOptions {
    pub fn get(&self, key: &str) -> Option<&toml::Value> {
        self.0.get(key)
    }
}

type Cached = Arc<dyn Any + Send + Sync>;

/// The `[rules.<ID>]` option tables of a scan, with each typed parse cached.
#[derive(Debug, Default)]
pub struct OptionsStore {
    raw: HashMap<String, RuleOptions>,
    /// Path named in option errors.
    config_path: PathBuf,
    typed: RwLock<HashMap<(String, TypeId), Cached>>,
}

impl OptionsStore {
    /// A store over `raw` (rule ID → options); errors name `config_path`.
    pub fn new(raw: HashMap<String, RuleOptions>, config_path: impl Into<PathBuf>) -> Self {
        Self {
            raw,
            config_path: config_path.into(),
            typed: RwLock::default(),
        }
    }

    /// A store with no options, naming the default `pulse.toml` in errors.
    pub fn empty() -> Self {
        Self::new(HashMap::new(), CONFIG_FILE)
    }

    /// Typed options of `rule_id`; a missing table yields `T::default()`.
    pub fn get<T>(&self, rule_id: &str) -> Result<T, ConfigError>
    where
        T: DeserializeOwned + Default + Clone + Send + Sync + 'static,
    {
        let key = (rule_id.to_owned(), TypeId::of::<T>());
        let cached = self
            .typed
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&key)
            .cloned();
        if let Some(any) = cached.and_then(|c| c.downcast::<T>().ok()) {
            return Ok(T::clone(&any));
        }
        let value: T =
            match self.raw.get(rule_id) {
                None => T::default(),
                Some(opts) => toml::Value::Table(opts.0.clone()).try_into().map_err(
                    |e: toml::de::Error| ConfigError::Invalid {
                        path: self.config_path.clone(),
                        message: format!("[rules.{rule_id}]: {}", e.message().trim()),
                    },
                )?,
            };
        self.typed
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(key, Arc::new(value.clone()));
        Ok(value)
    }
}

/// Everything a rule may consult besides the asset itself.
#[derive(Debug, Clone, Copy)]
pub struct RuleCtx<'a> {
    pub options: &'a OptionsStore,
    pub root: &'a Path,
    /// Every walked file, as a scan-root-relative path with `/` separators.
    pub files: &'a HashSet<String>,
}

impl RuleCtx<'_> {
    /// Typed options for `rule_id` from `[rules.<ID>]`, parsed once per scan. Option types
    /// use `#[serde(default, deny_unknown_fields)]`, so unknown keys are errors.
    pub fn options<T>(&self, rule_id: &str) -> Result<T, ConfigError>
    where
        T: DeserializeOwned + Default + Clone + Send + Sync + 'static,
    {
        self.options.get(rule_id)
    }

    /// Whether `rel_path` (scan-root-relative, `/` separators) was seen by the walker.
    pub fn file_exists(&self, rel_path: &str) -> bool {
        self.files.contains(rel_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Clone, Default, PartialEq, Deserialize)]
    #[serde(default, deny_unknown_fields)]
    struct Opts {
        max: u32,
        words: Vec<String>,
    }

    fn store(src: &str) -> OptionsStore {
        let table: toml::Table = toml::from_str(src).unwrap();
        let raw = HashMap::from([("AI-X-001".to_owned(), RuleOptions(table))]);
        OptionsStore::new(raw, "pulse.toml")
    }

    #[test]
    fn options_parse_typed_and_cache() {
        let s = store("max = 3\nwords = ['a']");
        let files = HashSet::new();
        let ctx = RuleCtx {
            options: &s,
            root: Path::new("."),
            files: &files,
        };
        let o: Opts = ctx.options("AI-X-001").unwrap();
        assert_eq!(
            o,
            Opts {
                max: 3,
                words: vec!["a".into()]
            }
        );
        assert_eq!(ctx.options::<Opts>("AI-X-001").unwrap(), o);
        assert_eq!(ctx.options::<Opts>("AI-Y-002").unwrap(), Opts::default());
    }

    #[test]
    fn options_unknown_key_is_error() {
        let err = store("maxx = 3").get::<Opts>("AI-X-001").unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("[rules.AI-X-001]") && msg.contains("maxx"),
            "{msg}"
        );
    }
}
