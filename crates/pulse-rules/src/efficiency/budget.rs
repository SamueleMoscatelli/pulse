//! Token budgets: per asset (AI-EFF-001) and for the always-on context (AI-EFF-002).

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
        // P4.3: add the tokens of resolved `@imports` of root instructions.
        let budget = ctx.config.budgets.always_on;
        let total: u64 = assets.iter().map(|a| u64::from(a.always_tokens())).sum();
        if total <= budget {
            return;
        }
        let mut top: Vec<(u32, &Asset)> = assets
            .iter()
            .map(|a| (a.always_tokens(), a))
            .filter(|&(t, _)| t > 0)
            .collect();
        // Largest first; ties by path, so the result is deterministic.
        top.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.rel_path.cmp(&b.1.rel_path)));
        let Some(&(_, largest)) = top.first() else {
            return;
        };
        let list = top
            .iter()
            .take(TOP)
            .map(|(t, a)| format!("{} {t}", truncate(&a.rel_path, 30)))
            .collect::<Vec<_>>()
            .join(", ");
        let more = if top.len() > TOP { ", …" } else { "" };
        let mut f = Finding::new(
            &META_ALWAYS,
            largest,
            Span::new(0, 0),
            format!("Always-on context {total} tokens > {budget}; top: {list}{more}"),
        );
        f.token_waste = u32::try_from(total - budget).unwrap_or(u32::MAX);
        out.push(f);
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
