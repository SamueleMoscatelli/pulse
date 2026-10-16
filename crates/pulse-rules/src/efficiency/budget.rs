//! Token budgets: per asset (AI-EFF-001) and for the always-on context (AI-EFF-002).

use std::collections::{HashMap, HashSet};

use pulse_core::refs::resolve;
use pulse_core::{
    Asset, AssetKind, Category, Finding, KindSet, ProjectRule, Rule, RuleCtx, RuleMeta, Severity,
    Span, config::Budgets,
};

use crate::util::truncate;

pub static META_ASSET: RuleMeta = RuleMeta {
    id: "AI-EFF-001",
    name: "Asset over token budget",
    category: Category::Efficiency,
    default_severity: Severity::High,
    // MCP configs, settings and hooks have no token budget.
    kinds: KindSet::ALL
        .without(AssetKind::McpConfig)
        .without(AssetKind::Settings)
        .without(AssetKind::Hooks),
    summary: "Asset is larger than the token budget for its kind.",
    remediation: "Trim or split the file, or move detail into on-demand files (skills, docs).",
    inline_suppressible: true,
};

pub struct AssetBudget;

/// The `[budgets]` entry for `kind`; `None` for kinds without a token budget.
fn budget(budgets: &Budgets, kind: AssetKind) -> Option<u64> {
    Some(match kind {
        AssetKind::Instructions => budgets.instructions,
        AssetKind::ScopedInstructions => budgets.scoped_instructions,
        AssetKind::Agent => budgets.agent,
        AssetKind::Prompt => budgets.prompt,
        AssetKind::Skill => budgets.skill,
        AssetKind::Tools => budgets.tools,
        AssetKind::Custom => budgets.custom,
        AssetKind::McpConfig | AssetKind::Settings | AssetKind::Hooks => return None,
    })
}

impl Rule for AssetBudget {
    fn meta(&self) -> &'static RuleMeta {
        &META_ASSET
    }

    fn check(&self, asset: &Asset, ctx: &RuleCtx<'_>, out: &mut Vec<Finding>) {
        let Some(budget) = budget(&ctx.config.budgets, asset.kind) else {
            return;
        };
        // A skill's budget covers its OnDemand body; the description is AI-EFF-002's.
        let tokens = u64::from(if asset.kind == AssetKind::Skill {
            asset.body_tokens()
        } else {
            asset.tokens()
        });
        if tokens <= budget {
            return;
        }
        let waste = tokens - budget;
        let mut f = Finding::new(
            &META_ASSET,
            asset,
            Span::new(0, 0),
            format!(
                "Over token budget: {tokens} tokens > {budget} for {} (+{waste})",
                asset.kind.as_str()
            ),
        );
        // Medium while within 125% of the budget (tokens * 4 <= budget * 5).
        if tokens.saturating_mul(4) <= budget.saturating_mul(5) {
            f.severity = Severity::Medium;
        }
        f.token_waste = u32::try_from(waste).unwrap_or(u32::MAX);
        out.push(f);
    }
}

pub static META_ALWAYS: RuleMeta = RuleMeta {
    id: "AI-EFF-002",
    name: "Always-on context over budget",
    category: Category::Efficiency,
    default_severity: Severity::High,
    kinds: KindSet::ALL,
    summary: "The context loaded into every session is larger than budgets.always_on.",
    remediation: "Shrink the largest always-on files, or make them scoped or on-demand.",
    inline_suppressible: true,
};

/// How many contributors the AI-EFF-002 message lists.
const TOP: usize = 5;

pub struct AlwaysOnBudget;

impl ProjectRule for AlwaysOnBudget {
    fn meta(&self) -> &'static RuleMeta {
        &META_ALWAYS
    }

    fn check_project(&self, assets: &[Asset], ctx: &RuleCtx<'_>, out: &mut Vec<Finding>) {
        let budget = ctx.config.budgets.always_on;
        let mut top: Vec<Contributor<'_>> = assets
            .iter()
            .map(|a| (a.always_tokens(), a.rel_path.as_str(), a, Span::new(0, 0)))
            .filter(|&(t, ..)| t > 0)
            .collect();
        imported(assets, ctx, &mut top);
        let total: u64 = top.iter().map(|c| u64::from(c.0)).sum();
        if total <= budget {
            return;
        }
        // Largest first; ties by path, so the result is deterministic.
        top.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(b.1)));
        let Some(&(_, _, anchor, span)) = top.first() else {
            return;
        };
        let list = top
            .iter()
            .take(TOP)
            .map(|(t, rel, ..)| format!("{} {t}", truncate(rel, 30)))
            .collect::<Vec<_>>()
            .join(", ");
        let more = if top.len() > TOP { ", …" } else { "" };
        let mut f = Finding::new(
            &META_ALWAYS,
            anchor,
            span,
            format!("Always-on context {total} tokens > {budget}; top: {list}{more}"),
        );
        f.token_waste = u32::try_from(total - budget).unwrap_or(u32::MAX);
        out.push(f);
    }
}

/// Always-on tokens of one file: `(tokens, path, asset the finding points at, span)`.
type Contributor<'a> = (u32, &'a str, &'a Asset, Span);

/// Adds the files that always-loaded instructions `@import` (transitively), each once,
/// minus what they already contribute as assets. An imported file is reported at the
/// import of the first root that reaches it.
fn imported<'a>(assets: &'a [Asset], ctx: &RuleCtx<'a>, out: &mut Vec<Contributor<'a>>) {
    let by_path: HashMap<&str, &Asset> = assets.iter().map(|a| (a.rel_path.as_str(), a)).collect();
    let mut roots: Vec<&Asset> = assets
        .iter()
        .filter(|a| {
            matches!(
                a.kind,
                AssetKind::Instructions | AssetKind::ScopedInstructions
            ) && a.always_tokens() > 0
        })
        .collect();
    roots.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    let mut seen: HashSet<&str> = roots.iter().map(|a| a.rel_path.as_str()).collect();
    for root in roots {
        for (rel, via) in ctx.imports.reachable(&root.rel_path) {
            if !seen.insert(rel) {
                continue;
            }
            let tokens = match (by_path.get(rel), ctx.imports.file(rel)) {
                (Some(a), _) => a.tokens().saturating_sub(a.always_tokens()),
                (None, Some(f)) => f.tokens(),
                (None, None) => 0,
            };
            if tokens == 0 {
                continue;
            }
            let span = root
                .markdown()
                .imports
                .iter()
                .find(|i| resolve(&root.rel_path, &i.path).as_deref() == Some(via))
                .map_or(Span::new(0, 0), |i| i.span);
            out.push((tokens, rel, root, span));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_in_meta_has_a_budget() {
        let b = Budgets::default();
        for kind in META_ASSET.kinds.iter() {
            assert!(budget(&b, kind).is_some(), "{kind:?}");
        }
    }
}
