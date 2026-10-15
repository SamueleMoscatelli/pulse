//! Pulse built-in analysis rules.

use pulse_core::{ProjectRule, Rule, RuleMeta};

pub mod cognitive;
pub mod efficiency;
pub mod lexicon;
pub mod security;
pub mod structure;
pub mod util;

/// Asset-scoped rules, in ID order.
pub static RULES: &[&dyn Rule] = &[
    &cognitive::contradiction::ContradictoryDirectives,
    &efficiency::budget::AssetBudget,
    &security::unicode::InvisibleUnicode,
    &security::injection::InjectionPhrase,
    &security::secrets::HardcodedSecret,
    &security::mcp::LiteralMcpCredential,
    &security::mcp::UnpinnedMcpPackage,
    &security::mcp::InsecureMcpTransport,
    &structure::content::InvalidContent,
];

/// Project-scoped rules, in ID order.
pub static PROJECT_RULES: &[&dyn ProjectRule] = &[&efficiency::budget::AlwaysOnBudget];

/// Metadata of every registered rule, asset rules first, each group in ID order.
pub fn all_meta() -> impl Iterator<Item = &'static RuleMeta> {
    RULES
        .iter()
        .map(|r| r.meta())
        .chain(PROJECT_RULES.iter().map(|r| r.meta()))
}

/// The registered rule with this exact ID.
pub fn find(id: &str) -> Option<&'static RuleMeta> {
    all_meta().find(|m| m.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_ids_are_unique_sorted_and_findable() {
        let ids: Vec<_> = all_meta().map(|m| m.id).collect();
        for group in [
            RULES.iter().map(|r| r.meta().id).collect::<Vec<_>>(),
            PROJECT_RULES.iter().map(|r| r.meta().id).collect(),
        ] {
            assert!(group.windows(2).all(|w| w[0] < w[1]), "{group:?}");
        }
        let mut dedup = ids.clone();
        dedup.sort_unstable();
        dedup.dedup();
        assert_eq!(dedup.len(), ids.len());
        assert_eq!(find("AI-STR-002").map(|m| m.id), Some("AI-STR-002"));
        assert!(find("AI-STR-999").is_none());
    }

    #[test]
    fn meta_ids_match_category_prefix() {
        for m in all_meta() {
            assert!(
                m.id.starts_with(&format!("AI-{}-", m.category.prefix())),
                "{}",
                m.id
            );
        }
    }
}
