//! Declarative custom rules (`[[custom_rules]]` in `pulse.toml`), compiled at scan start.

use std::collections::HashSet;
use std::sync::{LazyLock, Mutex, PoisonError};

use aho_corasick::{AhoCorasick, MatchKind};
use pulse_core::config::{CONFIG_FILE, CustomRule};
use pulse_core::{Asset, Category, ConfigError, Finding, KindSet, Rule, RuleCtx, RuleMeta, Span};
use regex::Regex;

use crate::lexicon::normalize_phrase;
use crate::security::injection::at_word_bounds;

/// A compiled `[[custom_rules]]` entry.
pub struct Custom {
    meta: &'static RuleMeta,
    message: &'static str,
    remediation: Option<&'static str>,
    matcher: Matcher,
}

enum Matcher {
    Pattern(Regex),
    Phrases(AhoCorasick),
    MaxTokens(u32),
}

/// Compiles `rules` in config order; a bad regex or phrase list is a config error naming
/// the rule ID. Assumes `Config` validation passed (one matcher each).
pub fn compile(rules: &[CustomRule]) -> Result<Vec<Custom>, ConfigError> {
    rules.iter().map(compile_one).collect()
}

fn compile_one(rule: &CustomRule) -> Result<Custom, ConfigError> {
    let invalid = |message: String| ConfigError::Invalid {
        path: CONFIG_FILE.into(),
        message: format!("custom rule `{}`: {message}", rule.id),
    };
    let matcher = if let Some(pattern) = &rule.pattern {
        Matcher::Pattern(
            Regex::new(pattern).map_err(|e| invalid(format!("invalid `pattern`: {e}")))?,
        )
    } else if let Some(phrases) = &rule.phrases {
        let normalized: Vec<String> = phrases.iter().map(|p| normalize_phrase(p)).collect();
        // An empty phrase would match at every position.
        if let Some(i) = normalized.iter().position(String::is_empty) {
            return Err(invalid(format!(
                "phrase \"{}\" is empty after normalization",
                phrases[i]
            )));
        }
        Matcher::Phrases(
            AhoCorasick::builder()
                .match_kind(MatchKind::LeftmostLongest)
                .build(&normalized)
                .map_err(|e| invalid(format!("invalid `phrases`: {e}")))?,
        )
    } else if let Some(max) = rule.max_tokens {
        Matcher::MaxTokens(max)
    } else {
        return Err(invalid(
            "needs exactly one of `pattern`, `phrases`, `max_tokens`".to_owned(),
        ));
    };
    let kinds = if rule.kinds.is_empty() {
        KindSet::ALL
    } else {
        rule.kinds
            .iter()
            .fold(KindSet::EMPTY, |set, &k| set.union(KindSet::of(k)))
    };
    let message = intern(&rule.message);
    let remediation = rule.remediation.as_deref().map(intern);
    let meta = intern_meta(RuleMeta {
        id: intern(&rule.id),
        name: intern(&rule.name),
        // Unused: custom rules have no category (`category_of` returns `None` for `X-` IDs).
        category: Category::Structure,
        default_severity: rule.severity,
        kinds,
        summary: message,
        remediation: remediation.unwrap_or(""),
        inline_suppressible: true,
    });
    Ok(Custom {
        meta,
        message,
        remediation,
        matcher,
    })
}

impl Custom {
    fn finding(&self, asset: &Asset, span: Span, message: String) -> Finding {
        let mut f = Finding::new(self.meta, asset, span, message);
        f.remediation = self.remediation;
        f
    }
}

impl Rule for Custom {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn check(&self, asset: &Asset, _ctx: &RuleCtx<'_>, out: &mut Vec<Finding>) {
        match &self.matcher {
            // The match text is not quoted: patterns may target secrets.
            Matcher::Pattern(re) => {
                for m in re.find_iter(&asset.source).filter(|m| !m.is_empty()) {
                    let span = Span::new(offset(m.start()), offset(m.end()));
                    out.push(self.finding(asset, span, self.message.to_owned()));
                }
            }
            Matcher::Phrases(ac) => {
                let norm = asset.normalized();
                let text = norm.text.as_str();
                for m in ac.find_iter(text) {
                    if at_word_bounds(text, m.range()) {
                        let span = norm.source_span(m.range());
                        out.push(self.finding(asset, span, self.message.to_owned()));
                    }
                }
            }
            Matcher::MaxTokens(max) => {
                let tokens = asset.tokens();
                if tokens > *max {
                    let msg = format!("{} ({tokens} tokens > {max})", self.message);
                    out.push(self.finding(asset, Span::new(0, 0), msg));
                }
            }
        }
    }
}

fn offset(i: usize) -> u32 {
    u32::try_from(i).unwrap_or(u32::MAX)
}

/// Rules expose `&'static` metadata; custom rules leak theirs once per distinct value, so
/// repeated scans with the same config don't grow memory.
fn intern(s: &str) -> &'static str {
    static STRINGS: LazyLock<Mutex<HashSet<&'static str>>> = LazyLock::new(Mutex::default);
    let mut set = STRINGS.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(&s) = set.get(s) {
        return s;
    }
    let leaked: &'static str = Box::leak(s.to_owned().into_boxed_str());
    set.insert(leaked);
    leaked
}

fn intern_meta(meta: RuleMeta) -> &'static RuleMeta {
    static METAS: Mutex<Vec<&'static RuleMeta>> = Mutex::new(Vec::new());
    let mut metas = METAS.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(&m) = metas.iter().find(|m| ***m == meta) {
        return m;
    }
    let leaked: &'static RuleMeta = Box::leak(Box::new(meta));
    metas.push(leaked);
    leaked
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::Path;

    use pulse_core::{AssetKind, Config, ImportGraph, LoadMode, OptionsStore};

    use super::*;

    fn rules(src: &str) -> Result<Vec<Custom>, ConfigError> {
        let config = Config::parse(src, Path::new("pulse.toml")).unwrap();
        compile(&config.custom_rules)
    }

    fn run(rule: &Custom, kind: AssetKind, text: &str) -> Vec<Finding> {
        let asset = Asset::new(
            Path::new("/r/a.md"),
            "a.md",
            kind,
            LoadMode::Always,
            text.to_owned(),
        );
        let config = Config::default();
        let options = OptionsStore::empty();
        let files = HashSet::new();
        let ctx = RuleCtx {
            config: &config,
            options: &options,
            root: Path::new("/r"),
            files: &files,
            misplaced: &[],
            imports: &ImportGraph::default(),
        };
        let mut out = Vec::new();
        rule.check(&asset, &ctx, &mut out);
        out
    }

    const HEAD: &str = "[[custom_rules]]\nid = \"X-ORG-001\"\nname = \"n\"\nseverity = \"high\"\nmessage = \"Bad thing\"\n";

    #[test]
    fn pattern_matches_source_with_location() {
        let r = rules(&format!(
            "{HEAD}pattern = 'internal\\.corp'\nkinds = [\"skill\"]\n"
        ))
        .unwrap();
        assert_eq!(r[0].meta().kinds, KindSet::SKILL);
        let out = run(&r[0], AssetKind::Skill, "# T\nsee internal.corp now\n");
        assert_eq!(out.len(), 1);
        assert_eq!((out[0].location.line, out[0].location.col), (2, 5));
        assert_eq!(out[0].message, "Bad thing");
        assert_eq!(out[0].rule_id, "X-ORG-001");
        assert_eq!(out[0].severity, pulse_core::Severity::High);
        assert!(run(&r[0], AssetKind::Skill, "internal-corp").is_empty());
    }

    #[test]
    fn phrases_match_normalized_text_at_word_bounds() {
        let r = rules(&format!("{HEAD}phrases = [\"Ship It\"]\n")).unwrap();
        assert_eq!(r[0].meta().kinds, KindSet::ALL);
        let out = run(&r[0], AssetKind::Instructions, "Just **SHIP  it** today.\n");
        assert_eq!(out.len(), 1);
        assert!(run(&r[0], AssetKind::Instructions, "relationship items").is_empty());
    }

    #[test]
    fn max_tokens_reports_count() {
        let r = rules(&format!("{HEAD}max_tokens = 3\nremediation = \"Trim\"\n")).unwrap();
        let out = run(&r[0], AssetKind::Prompt, &"word ".repeat(50));
        assert_eq!(out.len(), 1);
        assert!(
            out[0].message.starts_with("Bad thing ("),
            "{}",
            out[0].message
        );
        assert_eq!(out[0].remediation, Some("Trim"));
        assert!(run(&r[0], AssetKind::Prompt, "hi").is_empty());
    }

    #[test]
    fn phrase_empty_after_normalization_is_error() {
        let err = rules(&format!("{HEAD}phrases = [\"ok\", \"**\"]\n"))
            .err()
            .unwrap();
        let msg = err.to_string();
        assert!(msg.contains("empty after normalization"), "{msg}");
    }

    #[test]
    fn bad_regex_names_rule() {
        let err = rules(&format!("{HEAD}pattern = '(unclosed'\n"))
            .err()
            .unwrap();
        let msg = err.to_string();
        assert!(
            msg.contains("custom rule `X-ORG-001`: invalid `pattern`"),
            "{msg}"
        );
    }

    #[test]
    fn metas_are_interned() {
        let src = format!("{HEAD}max_tokens = 3\n");
        let (a, b) = (rules(&src).unwrap(), rules(&src).unwrap());
        assert!(std::ptr::eq(a[0].meta(), b[0].meta()));
    }
}
