// Template for an asset-scoped rule. Adapt names to the real pulse-core API if it has evolved
// (check `crates/pulse-core/src/rule.rs`); keep the shape.
use pulse_core::{Asset, Category, Finding, KindSet, Rule, RuleCtx, RuleMeta, Severity};

pub static META: RuleMeta = RuleMeta {
    id: "AI-SEC-005",
    name: "Unpinned MCP server package",
    category: Category::Security,
    default_severity: Severity::High,
    kinds: KindSet::MCP_CONFIG,
    summary: "MCP server package is not pinned to an exact version.",
    remediation: "Pin an exact version (npx pkg@1.2.3, uvx pkg==1.2.3, image@sha256:...).",
    inline_suppressible: true,
};

pub struct UnpinnedMcpPackage;

impl Rule for UnpinnedMcpPackage {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, asset: &Asset, _ctx: &RuleCtx, out: &mut Vec<Finding>) {
        let Some(mcp) = asset.mcp() else { return };
        for server in &mcp.servers {
            // ... detection; build the message lazily, only on a hit:
            // out.push(Finding::new(&META, asset, span, format!("Unpinned MCP package \"{pkg}\"")));
            let _ = server;
        }
    }
}

#[cfg(test)]
mod tests {
    // Unit-test helpers here (parsers, matchers). Behavior is covered by fixtures.
}
