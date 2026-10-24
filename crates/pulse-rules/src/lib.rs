//! Pulse built-in analysis rules.

use pulse_core::{ProjectRule, Rule, RuleMeta};

pub mod cognitive;
pub mod custom;
mod docs;
pub mod efficiency;
pub mod lexicon;
pub mod security;
pub mod structure;
pub mod util;

pub use docs::docs;

/// Asset-scoped rules, in ID order.
pub static RULES: &[&dyn Rule] = &[
    &cognitive::contradiction::ContradictoryDirectives,
    &cognitive::style::VagueDirective,
    &cognitive::style::NegativeOnly,
    &cognitive::style::EmphasisInflation,
    &cognitive::routing::WeakRouting,
    &cognitive::layout::Unstructured,
    &efficiency::budget::AssetBudget,
    &efficiency::bloat::InlinedBulk,
    &efficiency::bloat::Oversized,
    &efficiency::filler::FillerPhrasing,
    &security::unicode::InvisibleUnicode,
    &security::injection::InjectionPhrase,
    &security::secrets::HardcodedSecret,
    &security::mcp::LiteralMcpCredential,
    &security::mcp::UnpinnedMcpPackage,
    &security::mcp::InsecureMcpTransport,
    &security::permissions::OverPermissive,
    &security::hooks::HookRemoteExec,
    &security::hooks::HookUnsafeInterpolation,
    &security::exfil::Exfiltration,
    &security::hidden::HiddenInstructions,
    &security::poisoning::ToolPoisoning,
    &security::paths::SensitivePath,
    &structure::frontmatter::InvalidFrontmatter,
    &structure::content::InvalidContent,
    &structure::frontmatter::NameDescription,
    &structure::hooks::InvalidHook,
];

/// Project-scoped rules, in ID order.
pub static PROJECT_RULES: &[&dyn ProjectRule] = &[
    &cognitive::contradiction::CrossFileContradiction,
    &efficiency::budget::AlwaysOnBudget,
    &efficiency::duplicates::DuplicatedContent,
    &efficiency::bloat::McpSprawl,
    &structure::refs::BrokenReference,
    &structure::tools::UnknownTool,
    &structure::names::DuplicateName,
];

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

/// Starts building the costly lexicon automatons and regexes on the rayon pool, in
/// parallel, without waiting (P7.1, ADR-039). Built lazily on first use instead, they
/// cost ~13 ms per run serially on the first assets; the engine calls this before
/// discovery, while the pool would otherwise sit idle during the walk.
pub fn warm_up() {
    use std::sync::LazyLock;
    let inits: [fn(); 8] = [
        || {
            LazyLock::force(&lexicon::secrets::MATCHERS);
        },
        || {
            LazyLock::force(&lexicon::injection::INJECTION);
        },
        || {
            LazyLock::force(&lexicon::poisoning::TOOL_CALL);
        },
        || {
            LazyLock::force(&lexicon::style::FILLER_SET);
        },
        || {
            LazyLock::force(&lexicon::poisoning::CONCEAL);
        },
        || {
            LazyLock::force(&lexicon::paths::PATHS);
        },
        || {
            LazyLock::force(&lexicon::mcp::CREDENTIAL_KEY);
        },
        || {
            LazyLock::force(&lexicon::poisoning::HIDDEN_TAG);
        },
    ];
    for init in inits {
        rayon::spawn(init);
    }
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
