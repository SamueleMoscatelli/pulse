//! Metrics and A–E ratings (spec 04-scoring-gate → Ratings, Metrics).

use std::collections::BTreeMap;

use pulse_core::{AssetKind, Assistant, Category, Finding, Rating, Severity};

use crate::AssetMeta;

/// Scan-wide numbers; computed on open findings (after suppressions).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Metrics {
    pub total_tokens: u64,
    /// Always-on tokens of the assistant that loads the most (see [`ContextLoad`]).
    pub always_on_tokens: u64,
    /// Deduplicated EFF token waste over `total_tokens`, capped at 1.0.
    pub context_debt: f64,
    /// `always_on_tokens / context.window`, as a fraction (not capped).
    pub context_load: f64,
    pub issues_by_severity: BTreeMap<Severity, usize>,
    /// Findings of custom (`X-`) rules have no category and are not counted here.
    pub issues_by_category: BTreeMap<Category, usize>,
    pub assets_by_kind: BTreeMap<AssetKind, usize>,
}

/// What the assistants load on every request (spec 04-scoring-gate → Metrics, ADR-045).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContextLoad {
    /// Always-on tokens per assistant whose always-on assets are present, largest first.
    pub by_assistant: Vec<(Assistant, u64)>,
    /// The largest per-assistant total; the sum of all always-on tokens when no file names
    /// an assistant.
    pub always_on_tokens: u64,
    /// What the largest assistant (or every one) loads, largest first.
    pub contributors: Vec<Contributor>,
    /// Tokens outside the always-on part: skill and agent bodies, prompts, scoped rules.
    pub on_demand_tokens: u64,
    /// `context.window`.
    pub window: u64,
    /// `budgets.always_on`.
    pub budget: u64,
}

impl ContextLoad {
    /// `always_on_tokens / window`, as a fraction (not capped).
    pub fn ratio(&self) -> f64 {
        self.always_on_tokens as f64 / self.window.max(1) as f64
    }
}

/// One entry of the always-loaded list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contributor {
    pub source: Source,
    pub tokens: u64,
}

/// A file, or the descriptions of every skill or agent, which are listed as one entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    File(String),
    SkillDescriptions(usize),
    AgentDescriptions(usize),
}

/// Computes the [`ContextLoad`] of the in-scope assets.
pub fn context_load(assets: &[&AssetMeta], window: u64, budget: u64) -> ContextLoad {
    let always: Vec<(&AssetMeta, Option<pulse_core::AssistantSet>)> = assets
        .iter()
        .filter(|a| a.always_tokens > 0)
        .map(|&a| (a, pulse_core::loaded_by(&a.rel_path, a.kind)))
        .collect();
    let (by_assistant, always_on_tokens) =
        pulse_core::per_assistant(always.iter().map(|(a, s)| (*s, u64::from(a.always_tokens))));
    let largest = by_assistant.first().map(|&(a, _)| a);
    let (mut skills, mut agents) = ((0, 0), (0, 0));
    let mut contributors = Vec::new();
    for (a, set) in &always {
        if largest.is_some_and(|l| set.is_some_and(|s| !s.contains(l))) {
            continue;
        }
        let tokens = u64::from(a.always_tokens);
        match a.kind {
            AssetKind::Skill => skills = (skills.0 + 1, skills.1 + tokens),
            AssetKind::Agent => agents = (agents.0 + 1, agents.1 + tokens),
            _ => contributors.push(Contributor {
                source: Source::File(a.rel_path.clone()),
                tokens,
            }),
        }
    }
    for ((n, tokens), source) in [
        (skills, Source::SkillDescriptions as fn(usize) -> Source),
        (agents, Source::AgentDescriptions),
    ] {
        if n > 0 {
            contributors.push(Contributor {
                source: source(n),
                tokens,
            });
        }
    }
    // Largest first; ties keep files (sorted by path) before the description groups.
    contributors.sort_by_key(|c| std::cmp::Reverse(c.tokens));
    let on_demand_tokens = assets
        .iter()
        .filter(|a| {
            !matches!(
                a.kind,
                AssetKind::McpConfig | AssetKind::Settings | AssetKind::Hooks
            )
        })
        .map(|a| u64::from(a.tokens.saturating_sub(a.always_tokens)))
        .sum();
    ContextLoad {
        by_assistant,
        always_on_tokens,
        contributors,
        on_demand_tokens,
        window,
        budget,
    }
}

/// Sonar-style ratings per dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ratings {
    pub security: Rating,
    pub reliability: Rating,
    pub clarity: Rating,
    pub efficiency: Rating,
}

impl Default for Ratings {
    fn default() -> Self {
        Self {
            security: Rating::A,
            reliability: Rating::A,
            clarity: Rating::A,
            efficiency: Rating::A,
        }
    }
}

/// Computes [`Metrics`] from open findings and the scanned assets.
pub fn metrics(findings: &[Finding], assets: &[&AssetMeta], load: &ContextLoad) -> Metrics {
    let total_tokens: u64 = assets.iter().map(|a| u64::from(a.tokens)).sum();
    let mut m = Metrics {
        total_tokens,
        always_on_tokens: load.always_on_tokens,
        context_debt: context_debt(findings, total_tokens),
        context_load: load.ratio(),
        ..Metrics::default()
    };
    for f in findings {
        *m.issues_by_severity.entry(f.severity).or_default() += 1;
        if let Some(c) = category_of(f.rule_id) {
            *m.issues_by_category.entry(c).or_default() += 1;
        }
    }
    for a in assets {
        *m.assets_by_kind.entry(a.kind).or_default() += 1;
    }
    m
}

/// Computes [`Ratings`] from open findings and the context debt ratio.
pub fn ratings(findings: &[Finding], context_debt: f64) -> Ratings {
    let worst = |cat: Category| {
        findings
            .iter()
            .filter(|f| category_of(f.rule_id) == Some(cat))
            .map(|f| f.severity)
            .max()
    };
    Ratings {
        security: severity_rating(worst(Category::Security)),
        reliability: severity_rating(worst(Category::Structure)),
        clarity: severity_rating(worst(Category::Cognitive)),
        efficiency: debt_rating(context_debt),
    }
}

/// Rating from the worst open severity in a category.
pub fn severity_rating(worst: Option<Severity>) -> Rating {
    match worst {
        None | Some(Severity::Info) => Rating::A,
        Some(Severity::Low) => Rating::B,
        Some(Severity::Medium) => Rating::C,
        Some(Severity::High) => Rating::D,
        Some(Severity::Critical) => Rating::E,
    }
}

/// Efficiency rating from the context debt ratio.
pub fn debt_rating(debt: f64) -> Rating {
    match debt {
        d if d <= 0.05 => Rating::A,
        d if d <= 0.10 => Rating::B,
        d if d <= 0.20 => Rating::C,
        d if d <= 0.50 => Rating::D,
        _ => Rating::E,
    }
}

/// The category of a built-in rule ID (`AI-<PREFIX>-NNN`); `None` for custom rules.
pub fn category_of(rule_id: &str) -> Option<Category> {
    let prefix = rule_id.strip_prefix("AI-")?.split('-').next()?;
    Category::ALL.iter().copied().find(|c| c.prefix() == prefix)
}

/// `min(1, waste / max(total, 1))`, where overlapping EFF waste in a file counts once
/// (the largest wins).
fn context_debt(findings: &[Finding], total_tokens: u64) -> f64 {
    let mut eff: Vec<&Finding> = findings
        .iter()
        .filter(|f| f.token_waste > 0 && category_of(f.rule_id) == Some(Category::Efficiency))
        .collect();
    eff.sort_by(|a, b| {
        let (a, b) = (&a.location, &b.location);
        (&a.path, a.line, a.col).cmp(&(&b.path, b.line, b.col))
    });
    let mut waste = 0u64;
    // Current cluster of overlapping findings: its path, start, furthest end and max waste.
    let mut cluster: Option<(&Finding, (u32, u32), u32)> = None;
    for f in eff {
        let l = &f.location;
        let (start, end) = ((l.line, l.col), (l.end_line, l.end_col));
        if let Some((first, cur_end, max)) = &mut cluster {
            let fl = &first.location;
            let same_start = start == (fl.line, fl.col);
            if fl.path == l.path && (start < *cur_end || same_start) {
                *cur_end = (*cur_end).max(end);
                *max = (*max).max(f.token_waste);
                continue;
            }
            waste += u64::from(*max);
        }
        cluster = Some((f, end, f.token_waste));
    }
    if let Some((_, _, max)) = cluster {
        waste += u64::from(max);
    }
    (waste as f64 / total_tokens.max(1) as f64).min(1.0)
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Arc;

    use pulse_core::{LoadMode, Location};

    use super::*;

    fn finding(rule_id: &'static str, sev: Severity, path: &str, span: [u32; 4]) -> Finding {
        Finding {
            rule_id,
            severity: sev,
            location: Location {
                path: Arc::from(Path::new(path)),
                line: span[0],
                col: span[1],
                end_line: span[2],
                end_col: span[3],
            },
            message: String::new(),
            remediation: None,
            token_waste: 0,
            fingerprint: 0,
        }
    }

    fn waste(path: &str, span: [u32; 4], waste: u32) -> Finding {
        Finding {
            token_waste: waste,
            ..finding("AI-EFF-001", Severity::Low, path, span)
        }
    }

    fn asset(kind: AssetKind, tokens: u32, always_tokens: u32) -> AssetMeta {
        AssetMeta {
            rel_path: String::new(),
            kind,
            load: LoadMode::Always,
            tokens,
            always_tokens,
        }
    }

    #[test]
    fn severity_rating_table() {
        let cases = [
            (None, Rating::A),
            (Some(Severity::Info), Rating::A),
            (Some(Severity::Low), Rating::B),
            (Some(Severity::Medium), Rating::C),
            (Some(Severity::High), Rating::D),
            (Some(Severity::Critical), Rating::E),
        ];
        for (worst, want) in cases {
            assert_eq!(severity_rating(worst), want, "{worst:?}");
        }
    }

    #[test]
    fn debt_rating_boundaries() {
        let cases = [
            (0.0, Rating::A),
            (0.05, Rating::A),
            (0.050_001, Rating::B),
            (0.10, Rating::B),
            (0.100_001, Rating::C),
            (0.20, Rating::C),
            (0.200_001, Rating::D),
            (0.50, Rating::D),
            (0.500_001, Rating::E),
            (1.0, Rating::E),
        ];
        for (debt, want) in cases {
            assert_eq!(debt_rating(debt), want, "{debt}");
        }
    }

    #[test]
    fn category_from_rule_id() {
        assert_eq!(category_of("AI-SEC-001"), Some(Category::Security));
        assert_eq!(category_of("AI-STR-002"), Some(Category::Structure));
        assert_eq!(category_of("AI-EFF-001"), Some(Category::Efficiency));
        assert_eq!(category_of("AI-COG-001"), Some(Category::Cognitive));
        assert_eq!(category_of("X-ACME-001"), None);
        assert_eq!(category_of("AI-FOO-001"), None);
    }

    #[test]
    fn ratings_use_worst_severity_per_category() {
        let p = "a.md";
        let s = [1, 1, 1, 2];
        let findings = [
            finding("AI-SEC-001", Severity::Low, p, s),
            finding("AI-SEC-002", Severity::Critical, p, s),
            finding("AI-STR-002", Severity::Medium, p, s),
            finding("X-ACME-001", Severity::Critical, p, s),
        ];
        let r = ratings(&findings, 0.07);
        assert_eq!(
            r,
            Ratings {
                security: Rating::E,
                reliability: Rating::C,
                clarity: Rating::A,
                efficiency: Rating::B,
            }
        );
    }

    #[test]
    fn debt_dedups_overlapping_waste() {
        // (findings, expected waste over 1000 tokens)
        let cases: Vec<(Vec<Finding>, u64)> = vec![
            (vec![], 0),
            // Disjoint spans add up.
            (
                vec![waste("a", [1, 1, 1, 10], 10), waste("a", [2, 1, 2, 10], 20)],
                30,
            ),
            // Same span counts once, the max.
            (
                vec![waste("a", [1, 1, 1, 10], 10), waste("a", [1, 1, 1, 10], 40)],
                40,
            ),
            // Partial overlap and a chain: one cluster.
            (
                vec![
                    waste("a", [1, 1, 3, 1], 5),
                    waste("a", [2, 5, 4, 1], 50),
                    waste("a", [3, 9, 5, 1], 7),
                ],
                50,
            ),
            // A zero-width finding inside a longer span overlaps it.
            (
                vec![waste("a", [1, 1, 3, 1], 30), waste("a", [2, 4, 2, 4], 8)],
                30,
            ),
            // Touching end/start (half-open) do not overlap.
            (
                vec![waste("a", [1, 1, 1, 5], 5), waste("a", [1, 5, 1, 9], 6)],
                11,
            ),
            // Same span in different files does not overlap.
            (
                vec![waste("a", [1, 1, 1, 10], 10), waste("b", [1, 1, 1, 10], 10)],
                20,
            ),
            // Non-EFF waste is ignored.
            (
                vec![Finding {
                    token_waste: 99,
                    ..finding("AI-SEC-001", Severity::High, "a", [1, 1, 1, 2])
                }],
                0,
            ),
        ];
        for (i, (findings, want)) in cases.into_iter().enumerate() {
            let got = context_debt(&findings, 1000);
            assert!(
                (got - want as f64 / 1000.0).abs() < 1e-12,
                "case {i}: {got}"
            );
        }
    }

    #[test]
    fn debt_is_capped_and_total_floored() {
        let f = [waste("a", [1, 1, 1, 2], 500)];
        assert_eq!(context_debt(&f, 100), 1.0);
        assert_eq!(context_debt(&f, 0), 1.0);
        assert_eq!(context_debt(&[], 0), 0.0);
    }

    #[test]
    fn metrics_aggregate() {
        let assets = [
            asset(AssetKind::Instructions, 1000, 1000),
            asset(AssetKind::Skill, 3000, 50),
            asset(AssetKind::Skill, 1000, 0),
        ];
        let findings = [
            waste("a", [1, 1, 1, 9], 250),
            finding("AI-SEC-001", Severity::High, "a", [1, 1, 1, 2]),
            finding("X-ACME-001", Severity::High, "a", [1, 1, 1, 2]),
        ];
        let scoped: Vec<&AssetMeta> = assets.iter().collect();
        let m = metrics(&findings, &scoped, &context_load(&scoped, 200_000, 8000));
        assert_eq!(m.total_tokens, 5000);
        assert_eq!(m.always_on_tokens, 1050);
        assert!((m.context_debt - 0.05).abs() < 1e-12);
        assert!((m.context_load - 1050.0 / 200_000.0).abs() < 1e-12);
        assert_eq!(
            m.issues_by_severity,
            BTreeMap::from([(Severity::Low, 1), (Severity::High, 2)])
        );
        assert_eq!(
            m.issues_by_category,
            BTreeMap::from([(Category::Security, 1), (Category::Efficiency, 1)])
        );
        assert_eq!(
            m.assets_by_kind,
            BTreeMap::from([(AssetKind::Instructions, 1), (AssetKind::Skill, 2)])
        );
    }

    fn at(rel: &str, kind: AssetKind, tokens: u32, always_tokens: u32) -> AssetMeta {
        AssetMeta {
            rel_path: rel.to_owned(),
            ..asset(kind, tokens, always_tokens)
        }
    }

    #[test]
    fn context_load_uses_the_largest_assistant() {
        let assets = [
            at("AGENTS.md", AssetKind::Instructions, 300, 300),
            at("CLAUDE.md", AssetKind::Instructions, 100, 100),
            at(".claude/skills/a/SKILL.md", AssetKind::Skill, 500, 20),
            at(".claude/skills/b/SKILL.md", AssetKind::Skill, 500, 30),
            at(".github/prompts/p.prompt.md", AssetKind::Prompt, 70, 0),
            at("agent.tools.json", AssetKind::Tools, 5, 5),
            at(".mcp.json", AssetKind::McpConfig, 40, 0),
        ];
        let scoped: Vec<&AssetMeta> = assets.iter().collect();
        let load = context_load(&scoped, 1000, 8000);
        use pulse_core::Assistant::*;
        assert_eq!(
            load.by_assistant,
            vec![
                (Copilot, 455),
                (Cursor, 305),
                (Windsurf, 305),
                (Codex, 305),
                (ClaudeCode, 155)
            ]
        );
        assert_eq!(load.always_on_tokens, 455);
        assert!((load.ratio() - 0.455).abs() < 1e-12);
        assert_eq!(load.on_demand_tokens, 950 + 70);
        let list: Vec<(Source, u64)> = load
            .contributors
            .into_iter()
            .map(|c| (c.source, c.tokens))
            .collect();
        assert_eq!(
            list,
            vec![
                (Source::File("AGENTS.md".into()), 300),
                (Source::File("CLAUDE.md".into()), 100),
                (Source::SkillDescriptions(2), 50),
                (Source::File("agent.tools.json".into()), 5),
            ]
        );
    }

    #[test]
    fn context_load_without_named_assistants_sums_everything() {
        let assets = [
            at("agent.tools.json", AssetKind::Tools, 5, 5),
            at(".claude/commands/c.md", AssetKind::Prompt, 70, 0),
        ];
        let scoped: Vec<&AssetMeta> = assets.iter().collect();
        let load = context_load(&scoped, 1000, 8000);
        assert!(load.by_assistant.is_empty());
        assert_eq!((load.always_on_tokens, load.on_demand_tokens), (5, 70));
    }
}
