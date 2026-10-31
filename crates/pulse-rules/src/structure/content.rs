//! AI-STR-002: invalid file content.

use pulse_core::parse::ViewError;
use pulse_core::{
    Asset, AssetKind, Category, Finding, KindSet, Rule, RuleCtx, RuleMeta, Severity, Span,
};

use crate::util::truncate;

pub static META: RuleMeta = RuleMeta {
    id: "AI-STR-002",
    name: "Invalid file content",
    category: Category::Structure,
    default_severity: Severity::High,
    kinds: KindSet::ALL,
    summary: "File is not valid UTF-8, JSON, YAML frontmatter, or has the wrong shape for its kind.",
    remediation: "Fix the syntax or shape so the agent (and Pulse) can read the whole file.",
    inline_suppressible: true,
};

/// Longest parser message quoted in a finding.
const MAX_MSG: usize = 70;

pub struct InvalidContent;

impl Rule for InvalidContent {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, asset: &Asset, _ctx: &RuleCtx<'_>, out: &mut Vec<Finding>) {
        if let Some(reason) = &asset.skipped {
            let mut f = Finding::new(
                &META,
                asset,
                Span::default(),
                format!("File skipped: {}", truncate(reason, MAX_MSG)),
            );
            f.severity = Severity::Info;
            out.push(f);
            return;
        }
        if asset.lossy {
            let at = asset.source.find('\u{FFFD}').unwrap_or(0);
            let at = u32::try_from(at).unwrap_or(0);
            out.push(Finding::new(
                &META,
                asset,
                Span::new(at, at + 1),
                "Invalid UTF-8: bytes replaced with U+FFFD",
            ));
        }
        match asset.kind {
            AssetKind::McpConfig => json(asset, &asset.mcp().errors, out),
            AssetKind::Settings | AssetKind::Hooks => json(asset, &asset.settings().errors, out),
            AssetKind::Tools => json(asset, &asset.tools().errors, out),
            AssetKind::Agent if asset.is_json_agent() => {
                // Inline `hooks` and `mcpServers`; the root's type is reported once.
                let mut shape = asset.settings().errors.clone();
                if let Some(agent) = asset.kiro_agent() {
                    shape.extend_from_slice(&agent.errors);
                }
                if asset
                    .json()
                    .root
                    .as_ref()
                    .is_some_and(|r| r.as_object().is_some())
                {
                    shape.extend_from_slice(&asset.mcp().errors);
                }
                json(asset, &shape, out);
            }
            kind if KindSet::MARKDOWN.contains(kind) => frontmatter(asset, out),
            _ => {}
        }
    }
}

/// Syntax errors of the JSON document, then shape errors of the kind's view.
fn json(asset: &Asset, shape: &[ViewError], out: &mut Vec<Finding>) {
    for e in &asset.json().errors {
        out.push(Finding::new(
            &META,
            asset,
            e.span,
            format!("Invalid JSON: {}", truncate(&e.msg, MAX_MSG)),
        ));
    }
    for e in shape {
        out.push(Finding::new(
            &META,
            asset,
            e.span,
            format!("Invalid shape: {}", truncate(&e.msg, MAX_MSG)),
        ));
    }
}

fn frontmatter(asset: &Asset, out: &mut Vec<Finding>) {
    let Some(Err(e)) = asset.markdown().frontmatter.as_ref().map(|f| &f.value) else {
        return;
    };
    let span = asset.lines.line_span(&asset.source, e.line);
    out.push(Finding::new(
        &META,
        asset,
        span,
        format!("Invalid YAML frontmatter: {}", truncate(&e.msg, MAX_MSG)),
    ));
}
