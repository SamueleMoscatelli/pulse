//! The scan pipeline (spec 01-architecture → Data flow).

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::time::Instant;

use pulse_core::config::{CONFIG_FILE, RuleSetting, did_you_mean};
use pulse_core::{
    Asset, Config, ConfigError, DiscoveredFile, Finding, OptionsStore, ProjectRule, Rule, RuleCtx,
    RuleOptions, Severity, discover,
};
use pulse_rules::{PROJECT_RULES, RULES, all_meta, structure};
use rayon::prelude::*;

use crate::post::{dedup, fingerprint, sort};
use crate::{AssetMeta, EngineError, Report, ScanOptions, Timings};

/// Scans `root` with `config`: discover, read, run rules, override severities, dedup,
/// fingerprint and sort.
pub fn scan(root: &Path, config: &Config, opts: &ScanOptions) -> Result<Report, EngineError> {
    let start = Instant::now();
    let rules = Resolved::new(config, opts)?;

    // Options are checked before the walk, so a bad `pulse.toml` fails fast.
    let no_files = HashSet::new();
    let early = RuleCtx {
        config,
        options: &rules.options,
        root,
        files: &no_files,
    };
    for rule in &rules.asset {
        rule.validate(&early)?;
    }
    for rule in &rules.project {
        rule.validate(&early)?;
    }

    let discovery = discover(root, config)?;
    let discovered_at = Instant::now();

    let max_bytes = config.scan.max_file_bytes;
    let calibration = config.context.calibration;
    let assets: Vec<Asset> = discovery
        .assets
        .par_iter()
        .map(|f| read(f, max_bytes, calibration))
        .collect();
    let read_at = Instant::now();

    let ctx = RuleCtx {
        config,
        options: &rules.options,
        root,
        files: &discovery.all_paths,
    };

    let mut findings: Vec<Finding> = assets
        .par_iter()
        .filter(|a| in_paths(&a.rel_path, &opts.paths))
        .flat_map_iter(|asset| {
            let mut out = Vec::new();
            for rule in &rules.asset {
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
        in_paths(&rel.to_string_lossy().replace('\\', "/"), &opts.paths)
    }));

    for f in &mut findings {
        if let Some(&sev) = rules.severity.get(f.rule_id) {
            f.severity = sev;
        }
    }
    let mut findings = dedup(findings);
    sort(&mut findings);
    fingerprint(&mut findings, &assets);
    let rules_at = Instant::now();

    let assets_meta = assets
        .iter()
        .map(|a| AssetMeta {
            rel_path: a.rel_path.clone(),
            kind: a.kind,
            load: a.effective_load(),
            tokens: a.tokens(),
            always_tokens: a.always_tokens(),
        })
        .collect();
    Ok(Report {
        findings,
        assets_meta,
        walk_errors: discovery.skipped,
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
    match res {
        Err(e) => skipped(format!("cannot read: {e}")),
        Ok(n) if n as u64 > max_bytes => {
            skipped(format!("over scan.max_file_bytes ({max_bytes} bytes)"))
        }
        Ok(_) => Asset::from_bytes(f.path.clone(), &*f.rel_path, f.kind, f.load, bytes)
            .with_calibration(calibration),
    }
}

/// Whether `rel` is one of `paths` or lies under one of them (empty `paths` keeps all).
fn in_paths(rel: &str, paths: &[String]) -> bool {
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
    severity: HashMap<&'static str, Severity>,
    options: OptionsStore,
}

impl Resolved {
    fn new(config: &Config, opts: &ScanOptions) -> Result<Self, ConfigError> {
        let mut options = HashMap::new();
        let mut severity = HashMap::new();
        let mut disabled = Vec::new();
        for (id, setting) in &config.rules {
            let Some(meta) = pulse_rules::find(id) else {
                if config.custom_rules.iter().any(|c| &c.id == id) {
                    continue;
                }
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
            severity,
            options: OptionsStore::new(options, CONFIG_FILE),
        })
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
        let config = Config::default();
        assert_eq!(
            ids(&Resolved::new(&config, &ScanOptions::default()).unwrap()),
            [
                "AI-COG-001",
                "AI-EFF-001",
                "AI-SEC-001",
                "AI-SEC-002",
                "AI-STR-002"
            ]
        );
        let skip = ScanOptions {
            skip_rules: vec!["AI-STR".into()],
            ..ScanOptions::default()
        };
        assert_eq!(
            ids(&Resolved::new(&config, &skip).unwrap()),
            ["AI-COG-001", "AI-EFF-001", "AI-SEC-001", "AI-SEC-002"]
        );
        let only = ScanOptions {
            only_rules: vec!["AI-SEC".into()],
            ..ScanOptions::default()
        };
        assert_eq!(
            ids(&Resolved::new(&config, &only).unwrap()),
            ["AI-SEC-001", "AI-SEC-002"]
        );
        let off = Config::parse("[rules.AI-STR-002]\nenabled = false\n", Path::new("p")).unwrap();
        assert_eq!(
            ids(&Resolved::new(&off, &ScanOptions::default()).unwrap()),
            ["AI-COG-001", "AI-EFF-001", "AI-SEC-001", "AI-SEC-002"]
        );
        let sev = Config::parse("[rules]\nAI-STR-002 = \"low\"\n", Path::new("p")).unwrap();
        let r = Resolved::new(&sev, &ScanOptions::default()).unwrap();
        assert_eq!(r.severity.get("AI-STR-002"), Some(&Severity::Low));
    }
}
