//! AI-STR-007: two skills, agents or prompts of one platform with the same invocation name.
//! Project scope: names collide across files.

use std::borrow::Cow;
use std::collections::HashMap;

use pulse_core::{
    Asset, AssetKind, Category, Finding, KindSet, ProjectRule, RuleCtx, RuleMeta, Severity, Span,
};

use crate::structure::frontmatter::key_span;
use crate::util::{tail, truncate};

pub static META: RuleMeta = RuleMeta {
    id: "AI-STR-007",
    name: "Duplicate asset name",
    category: Category::Structure,
    default_severity: Severity::High,
    kinds: KindSet::SKILL.union(KindSet::AGENT).union(KindSet::PROMPT),
    summary: "Two assets of one kind and platform share an invocation name; one shadows the other.",
    remediation: "Rename or remove one of the two assets.",
    inline_suppressible: true,
};

/// Chars of a name quoted in a message.
const QUOTED_NAME: usize = 24;
/// Chars of the first occurrence's path quoted in a message (its tail is kept).
const QUOTED_PATH: usize = 40;

/// Platform config directories; an asset under one shares its namespace with the others.
const PLATFORM_DIRS: &[&str] = &[
    ".agents",
    ".claude",
    ".codex",
    ".cursor",
    ".gemini",
    ".github",
    ".windsurf",
];

pub struct DuplicateName;

impl ProjectRule for DuplicateName {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_project(&self, assets: &[Asset], _ctx: &RuleCtx<'_>, out: &mut Vec<Finding>) {
        let mut ordered: Vec<&Asset> = assets
            .iter()
            .filter(|a| META.kinds.contains(a.kind) && a.skipped.is_none())
            .collect();
        // "First" is the first in path order, whatever order discovery produced.
        ordered.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
        let mut seen: HashMap<(AssetKind, &str, Cow<'_, str>), &Asset> = HashMap::new();
        for asset in ordered {
            let Some(name) = invocation_name(asset) else {
                continue;
            };
            let key = (asset.kind, namespace(&asset.rel_path), name.value);
            if let Some(first) = seen.get(&key) {
                let (label, prefix) = match asset.kind {
                    AssetKind::Skill => ("skill", ""),
                    AssetKind::Agent => ("agent", ""),
                    _ => ("prompt", "/"),
                };
                out.push(Finding::new(
                    &META,
                    asset,
                    name.span,
                    format!(
                        "Duplicate {label} name \"{prefix}{}\" (first: {})",
                        truncate(&key.2, QUOTED_NAME),
                        tail(&first.rel_path, QUOTED_PATH)
                    ),
                ));
            } else {
                seen.insert(key, asset);
            }
        }
    }
}

struct Name<'a> {
    value: Cow<'a, str>,
    /// The frontmatter `name` line, else the first line.
    span: Span,
}

/// The name an asset is invoked by: frontmatter `name`, else derived from its path
/// (skill folder, file stem, Claude command path `git/commit.md` → `git:commit`).
fn invocation_name(asset: &Asset) -> Option<Name<'_>> {
    let rel = asset.rel_path.as_str();
    if rel.ends_with(".prompty") {
        return None;
    }
    let first_line = || asset.lines.line_span(&asset.source, 1);
    // Claude command names come from the path only.
    if asset.kind == AssetKind::Prompt
        && let Some(i) = rel.find(".claude/commands/")
    {
        let cmd = rel[i + ".claude/commands/".len()..].strip_suffix(".md")?;
        return Some(Name {
            value: cmd.replace('/', ":").into(),
            span: first_line(),
        });
    }
    let fm = asset.markdown().frontmatter.as_ref();
    if let Some(fm) = fm
        && let Some(name) = fm.get("name").and_then(|v| v.as_str())
        && !name.trim().is_empty()
    {
        return Some(Name {
            value: name.trim().into(),
            span: key_span(asset, fm, "name"),
        });
    }
    let (dir, file) = rel.rsplit_once('/').unwrap_or(("", rel));
    let value = if asset.kind == AssetKind::Skill {
        dir.rsplit('/').next().filter(|d| !d.is_empty())?
    } else {
        [".agent.md", ".chatmode.md", ".prompt.md", ".md"]
            .iter()
            .find_map(|ext| file.strip_suffix(ext))
            .unwrap_or(file)
    };
    Some(Name {
        value: value.into(),
        span: first_line(),
    })
}

/// Where names must be unique: the platform directory (`.claude`, `.github`, …) when the
/// path has one, else the directory holding the asset's folder (`plugins/x/skills` for
/// `plugins/x/skills/y/SKILL.md`), so separate plugins don't collide.
fn namespace(rel: &str) -> &str {
    if let Some(dir) = rel.split('/').find(|seg| PLATFORM_DIRS.contains(seg)) {
        return dir;
    }
    let parent = rel.rsplit_once('/').map_or("", |(d, _)| d);
    parent.rsplit_once('/').map_or("", |(d, _)| d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namespaces() {
        assert_eq!(namespace(".claude/skills/a/SKILL.md"), ".claude");
        assert_eq!(namespace("pkg/.github/prompts/x.prompt.md"), ".github");
        assert_eq!(namespace("plugins/p/skills/a/SKILL.md"), "plugins/p/skills");
        assert_eq!(namespace("a/SKILL.md"), "");
    }
}
