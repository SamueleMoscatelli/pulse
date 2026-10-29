//! The scan pipeline (spec 01-architecture → Data flow).

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::time::Instant;

use pulse_core::config::{CONFIG_FILE, RuleSetting, did_you_mean};
use pulse_core::{
    Asset, Config, ConfigError, DiscoveredFile, Finding, ImportGraph, KindSet, OptionsStore,
    ProjectRule, Rule, RuleCtx, RuleMeta, RuleOptions, Severity, discover,
};
use pulse_rules::custom::{self, Custom};
use pulse_rules::{PROJECT_RULES, RULES, all_meta, structure};
use rayon::prelude::*;

use crate::post::{dedup, fingerprint, sort};
use crate::suppress::{self, Ignores};
use crate::{AssetMeta, EngineError, Report, ScanOptions, Timings};
use crate::{gate, score};

/// Scans `root` with `config`: discover, read, run rules, override severities, dedup,
/// fingerprint and sort.
pub fn scan(root: &Path, config: &Config, opts: &ScanOptions) -> Result<Report, EngineError> {
    let start = Instant::now();
    let rules = Resolved::new(config, opts)?;
    let ignores = Ignores::new(config)?;

    // Options are checked before the walk, so a bad `pulse.toml` fails fast.
    let no_files = HashSet::new();
    let no_imports = ImportGraph::default();
    let early = RuleCtx {
        config,
        options: &rules.options,
        root,
        files: &no_files,
        misplaced: &[],
        imports: &no_imports,
    };
    for rule in rules.asset_rules() {
        rule.validate(&early)?;
    }
    for rule in &rules.project {
        rule.validate(&early)?;
    }

    // The rule lexicons build on the rayon pool while the walk runs on its own threads.
    pulse_rules::warm_up();
    let discovery = discover(root, config)?;
    let discovered_at = Instant::now();

    let max_bytes = config.scan.max_file_bytes;
    let calibration = config.context.calibration;
    let assets: Vec<Asset> = discovery
        .assets
        .par_iter()
        .map(|f| {
            let asset = read(f, max_bytes, calibration);
            // Parsed here, in parallel: the serial import graph below needs every view.
            if KindSet::MARKDOWN.contains(asset.kind) {
                asset.markdown();
            }
            asset
        })
        .collect();
    let imports = ImportGraph::build(&assets, &discovery.all_paths, root, max_bytes, calibration);
    let read_at = Instant::now();

    let ctx = RuleCtx {
        config,
        options: &rules.options,
        root,
        files: &discovery.all_paths,
        misplaced: &discovery.misplaced,
        imports: &imports,
    };

    let mut findings: Vec<Finding> = assets
        .par_iter()
        .filter(|a| opts.in_scope(&a.rel_path))
        .flat_map_iter(|asset| {
            let mut out = Vec::new();
            for rule in rules.asset_rules() {
                let meta = rule.meta();
                // A skipped asset has no content: only AI-STR-002 reports it.
                if meta.kinds.contains(asset.kind)
                    && (asset.skipped.is_none() || meta.id == structure::content::META.id)
                {
                    rule.check(asset, &ctx, &mut out);
                }
            }
            out
        })
        .collect();
    // Project rules see every asset; their findings are filtered by path like the rest.
    let mut project = Vec::new();
    for rule in &rules.project {
        rule.check_project(&assets, &ctx, &mut project);
    }
    findings.extend(project.into_iter().filter(|f| {
        let rel = f
            .location
            .path
            .strip_prefix(root)
            .unwrap_or(&f.location.path);
        opts.in_scope(&rel.to_string_lossy().replace('\\', "/"))
    }));

    for f in &mut findings {
        if let Some(&sev) = rules.severity.get(f.rule_id) {
            f.severity = sev;
        }
    }
    let in_scope: Vec<&Asset> = assets
        .iter()
        .filter(|a| opts.in_scope(&a.rel_path))
        .collect();
    let ran = |id: &str| rules.runs(id);
    let (findings, suppressed) = suppress::apply(findings, &in_scope, root, &ignores, ran);
    let mut findings = dedup(findings);
    sort(&mut findings);
    fingerprint(&mut findings, &assets, root);
    let rules_at = Instant::now();

    let assets_meta: Vec<AssetMeta> = assets
        .iter()
        .map(|a| AssetMeta {
            rel_path: a.rel_path.clone(),
            kind: a.kind,
            load: a.effective_load(),
            tokens: a.tokens(),
            always_tokens: a.always_tokens(),
        })
        .collect();
    let scoped: Vec<&AssetMeta> = assets_meta
        .iter()
        .filter(|a| opts.in_scope(&a.rel_path))
        .collect();
    let window = config.context.window;
    let metrics = score::metrics(&findings, &scoped, window);
    let baseline = opts.baseline.clone();
    let is_new = |f: &Finding| baseline.as_ref().is_none_or(|b| b.is_new(f));
    let new_code = opts.new_only || config.gate.new_code_only;
    let new: Vec<Finding> = if new_code {
        findings.iter().filter(|f| is_new(f)).cloned().collect()
    } else {
        Vec::new()
    };
    let new_metrics = new_code.then(|| score::metrics(&new, &scoped, window));
    let (judged, judged_metrics) = match &new_metrics {
        Some(m) => (&new, m),
        None => (&findings, &metrics),
    };
    let ratings = score::ratings(judged, judged_metrics.context_debt);
    let gate = gate::evaluate(&config.gate, judged, judged_metrics, &ratings);
    let findings = if opts.new_only { new } else { findings };
    Ok(Report {
        findings,
        suppressed,
        assets_meta,
        walk_errors: discovery.skipped,
        metrics,
        new_metrics,
        ratings,
        gate,
        baseline,
        timings: Timings {
            discover: discovered_at - start,
            read: read_at - discovered_at,
            rules: rules_at - read_at,
            total: rules_at - start,
        },
    })
}

/// Reads one discovered file, at most `max_bytes`; oversized or unreadable files become
/// skipped assets.
fn read(f: &DiscoveredFile, max_bytes: u64, calibration: f64) -> Asset {
    let skipped =
        |reason: String| Asset::skipped(f.path.clone(), &*f.rel_path, f.kind, f.load, reason);
    let mut bytes = Vec::new();
    // One bounded read: a file growing after a size check can't bypass the cap.
    let res = File::open(&f.path).and_then(|file| {
        file.take(max_bytes.saturating_add(1))
            .read_to_end(&mut bytes)
    });
    let mut asset = match res {
        Err(e) => skipped(format!("cannot read: {e}")),
        Ok(n) if n as u64 > max_bytes => {
            skipped(format!("over scan.max_file_bytes ({max_bytes} bytes)"))
        }
        Ok(_) => Asset::from_bytes(f.path.clone(), &*f.rel_path, f.kind, f.load, bytes)
            .with_calibration(calibration),
    };
    asset.origin = f.origin;
    asset
}

/// A rule (built-in or custom) as `config` leaves it: effective severity and whether it runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleStatus {
    pub meta: &'static RuleMeta,
    pub severity: Severity,
    pub enabled: bool,
}

/// Every rule, built-in then custom, with its config overrides applied, in ID order.
pub fn rule_statuses(config: &Config) -> Result<Vec<RuleStatus>, ConfigError> {
    let resolved = Resolved::new(config, &ScanOptions::default())?;
    let mut statuses: Vec<_> = all_meta()
        .chain(resolved.custom_all.iter().map(|r| r.meta()))
        .map(|meta| RuleStatus {
            meta,
            severity: resolved
                .severity
                .get(meta.id)
                .copied()
                .unwrap_or(meta.default_severity),
            enabled: resolved.runs(meta.id),
        })
        .collect();
    statuses.sort_unstable_by_key(|s| s.meta.id);
    Ok(statuses)
}

/// Whether `rel` is one of `paths` or lies under one of them (empty `paths` keeps all).
pub(crate) fn in_paths(rel: &str, paths: &[String]) -> bool {
    paths.is_empty()
        || paths.iter().any(|p| {
            let p = p.trim_start_matches("./").trim_end_matches('/');
            p.is_empty()
                || p == "."
                || rel == p
                || rel.strip_prefix(p).is_some_and(|r| r.starts_with('/'))
        })
}

/// The rules to run and how, resolved from config and scan options.
struct Resolved {
    asset: Vec<&'static dyn Rule>,
    project: Vec<&'static dyn ProjectRule>,
    /// Every compiled custom rule, enabled or not (for `rule_statuses`).
    custom_all: Vec<Custom>,
    /// Indices into `custom_all` of the custom rules that run.
    custom: Vec<usize>,
    severity: HashMap<&'static str, Severity>,
    options: OptionsStore,
}

impl Resolved {
    fn new(config: &Config, opts: &ScanOptions) -> Result<Self, ConfigError> {
        let mut options = HashMap::new();
        let mut severity = HashMap::new();
        let mut disabled = Vec::new();
        let custom_all = custom::compile(&config.custom_rules)?;
        for (id, setting) in &config.rules {
            let found = pulse_rules::find(id)
                .or_else(|| custom_all.iter().map(|r| r.meta()).find(|m| m.id == id));
            let Some(meta) = found else {
                let ids = all_meta()
                    .map(|m| m.id)
                    .chain(config.custom_rules.iter().map(|c| c.id.as_str()));
                let hint = did_you_mean(id, ids)
                    .map_or_else(String::new, |s| format!(" (did you mean `{s}`?)"));
                return Err(ConfigError::Invalid {
                    path: CONFIG_FILE.into(),
                    message: format!("unknown rule `{id}` in [rules]{hint}"),
                });
            };
            match setting {
                RuleSetting::Off => disabled.push(meta.id),
                RuleSetting::Severity(s) => {
                    severity.insert(meta.id, *s);
                }
                RuleSetting::Options(t) => {
                    if t.enabled == Some(false) {
                        disabled.push(meta.id);
                    }
                    if let Some(s) = t.severity {
                        severity.insert(meta.id, s);
                    }
                    if !t.options.0.is_empty() && pulse_rules::find(meta.id).is_none() {
                        let key = t.options.0.keys().next().map_or("", String::as_str);
                        return Err(ConfigError::Invalid {
                            path: CONFIG_FILE.into(),
                            message: format!(
                                "[rules.{id}]: custom rules take no options (found `{key}`)"
                            ),
                        });
                    }
                    if !t.options.0.is_empty() {
                        options.insert(meta.id.to_owned(), RuleOptions(t.options.0.clone()));
                    }
                }
            }
        }
        let prefixed = |id: &str, list: &[String]| list.iter().any(|p| id.starts_with(p.as_str()));
        let enabled = |id: &str| {
            !disabled.contains(&id)
                && (opts.only_rules.is_empty() || prefixed(id, &opts.only_rules))
                && !prefixed(id, &opts.skip_rules)
        };
        Ok(Self {
            asset: RULES
                .iter()
                .copied()
                .filter(|r| enabled(r.meta().id))
                .collect(),
            project: PROJECT_RULES
                .iter()
                .copied()
                .filter(|r| enabled(r.meta().id))
                .collect(),
            custom: (0..custom_all.len())
                .filter(|&i| enabled(custom_all[i].meta().id))
                .collect(),
            custom_all,
            severity,
            options: OptionsStore::new(options, CONFIG_FILE),
        })
    }

    /// The asset rules that run: built-in, then custom.
    fn asset_rules(&self) -> impl Iterator<Item = &dyn Rule> {
        self.asset.iter().map(|r| *r as &dyn Rule).chain(
            self.custom
                .iter()
                .map(|&i| &self.custom_all[i] as &dyn Rule),
        )
    }

    /// Whether the rule `id` runs.
    fn runs(&self, id: &str) -> bool {
        self.asset_rules().any(|r| r.meta().id == id)
            || self.project.iter().any(|r| r.meta().id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_paths_matches_files_and_dirs() {
        let p = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert!(in_paths("a/b.md", &[]));
        assert!(in_paths("a/b.md", &p(&["a"])));
        assert!(in_paths("a/b.md", &p(&["./a/"])));
        assert!(in_paths("a/b.md", &p(&["a/b.md"])));
        assert!(in_paths("a/b.md", &p(&["."])));
        assert!(!in_paths("ab/c.md", &p(&["a"])));
        assert!(!in_paths("a/b.md", &p(&["a/b"])));
    }

    #[test]
    fn unknown_rule_id_suggests_closest() {
        let config =
            Config::parse("[rules]\nAI-STR-02 = \"off\"\n", Path::new("pulse.toml")).unwrap();
        let err = Resolved::new(&config, &ScanOptions::default())
            .err()
            .unwrap();
        let msg = err.to_string();
        assert!(
            msg.contains("unknown rule `AI-STR-02`") && msg.contains("did you mean `AI-STR-002`"),
            "{msg}"
        );
    }

    #[test]
    fn rule_selection_honors_config_and_prefixes() {
        let ids = |r: &Resolved| r.asset.iter().map(|r| r.meta().id).collect::<Vec<_>>();
        let all: Vec<&str> = pulse_rules::RULES.iter().map(|r| r.meta().id).collect();
        let except = |p: &str| {
            all.iter()
                .copied()
                .filter(|id| !id.starts_with(p))
                .collect::<Vec<_>>()
        };
        let config = Config::default();
        assert_eq!(
            ids(&Resolved::new(&config, &ScanOptions::default()).unwrap()),
            all
        );
        assert!(all.contains(&"AI-STR-002") && all.contains(&"AI-SEC-001"));
        let skip = ScanOptions {
            skip_rules: vec!["AI-STR".into()],
            ..ScanOptions::default()
        };
        assert_eq!(
            ids(&Resolved::new(&config, &skip).unwrap()),
            except("AI-STR")
        );
        let only = ScanOptions {
            only_rules: vec!["AI-SEC".into()],
            ..ScanOptions::default()
        };
        let sec: Vec<&str> = all
            .iter()
            .copied()
            .filter(|id| id.starts_with("AI-SEC"))
            .collect();
        assert_eq!(ids(&Resolved::new(&config, &only).unwrap()), sec);
        let off = Config::parse("[rules.AI-STR-002]\nenabled = false\n", Path::new("p")).unwrap();
        assert_eq!(
            ids(&Resolved::new(&off, &ScanOptions::default()).unwrap()),
            except("AI-STR-002")
        );
        let sev = Config::parse("[rules]\nAI-STR-002 = \"low\"\n", Path::new("p")).unwrap();
        let r = Resolved::new(&sev, &ScanOptions::default()).unwrap();
        assert_eq!(r.severity.get("AI-STR-002"), Some(&Severity::Low));
    }

    #[test]
    fn rule_statuses_apply_overrides_in_id_order() {
        let cfg = Config::parse(
            "[rules]\nAI-STR-002 = \"off\"\nAI-SEC-001 = \"low\"\n",
            Path::new("p"),
        )
        .unwrap();
        let statuses = rule_statuses(&cfg).unwrap();
        assert_eq!(statuses.len(), all_meta().count());
        assert!(statuses.windows(2).all(|w| w[0].meta.id < w[1].meta.id));
        let get = |id: &str| statuses.iter().find(|s| s.meta.id == id).unwrap();
        assert!(!get("AI-STR-002").enabled);
        assert_eq!(get("AI-SEC-001").severity, Severity::Low);
        assert!(get("AI-SEC-001").enabled);
        let cog = get("AI-COG-001");
        assert_eq!(cog.severity, cog.meta.default_severity);
        let bad = Config::parse("[rules]\nAI-SEC-999 = \"off\"\n", Path::new("p")).unwrap();
        assert!(rule_statuses(&bad).is_err());
    }
}
