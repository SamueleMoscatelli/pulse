//! AI-STR-008: an asset where no assistant loads it, so it silently does nothing.
//! Project scope: library-row assets are reported once per folder, and plugin folders are
//! recognized from the walked files (ADR-042).

use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use pulse_core::{
    Asset, AssetKind, Category, Finding, KindSet, Location, Misplaced, NearMiss, Origin,
    ProjectRule, RuleCtx, RuleMeta, Severity, Span,
};

use crate::util::tail;

pub static META: RuleMeta = RuleMeta {
    id: "AI-STR-008",
    name: "Asset not loaded from its location",
    category: Category::Structure,
    default_severity: Severity::Medium,
    kinds: KindSet::INSTRUCTIONS
        .union(KindSet::SCOPED_INSTRUCTIONS)
        .union(KindSet::SKILL)
        .union(KindSet::AGENT)
        .union(KindSet::PROMPT)
        .union(KindSet::HOOKS)
        .union(KindSet::MCP_CONFIG),
    summary: "An asset sits where no assistant loads it, so it silently does nothing.",
    remediation: "Move or rename it to the location named in the message, or declare a \
        distribution folder in `[scan] library`.",
    inline_suppressible: false,
};

/// Chars of a folder quoted in a message (its tail is kept).
const QUOTED_PATH: usize = 20;
/// Chars of a file name quoted in a message (its tail is kept).
const QUOTED_NAME: usize = 24;
/// Chars of an expected skill location quoted in a message (its tail is kept).
const QUOTED_EXPECTED: usize = 40;

/// Plugin manifests, relative to the plugin folder: assets under it load with the plugin.
const PLUGIN_MANIFESTS: &[&str] = &[
    "plugin.json",
    ".claude-plugin/plugin.json",
    ".github/plugin/plugin.json",
];

/// Platform folders that hold a `skills/` folder.
const SKILL_PLATFORMS: &[&str] = &[".agents", ".claude", ".github", ".kiro"];

/// How a library-row asset is named in a message, and where it belongs.
struct Group {
    one: &'static str,
    many: &'static str,
    expected: &'static str,
}

/// The group of a library-row asset; `kiro` for a `*.kiro.hook` file.
fn group(kind: AssetKind, kiro: bool) -> Option<Group> {
    let (one, many, expected) = match kind {
        AssetKind::Hooks if kiro => ("Kiro hook", "Kiro hooks", ".kiro/hooks/"),
        AssetKind::Agent => ("agent", "agents", ".github/agents/"),
        AssetKind::Prompt => ("prompt", "prompts", ".github/prompts/"),
        AssetKind::ScopedInstructions => {
            ("instruction file", "instructions", ".github/instructions/")
        }
        AssetKind::Hooks => ("hook config", "hook configs", ".github/hooks/"),
        AssetKind::McpConfig => ("MCP config", "MCP configs", ".vscode/mcp.json"),
        _ => return None,
    };
    Some(Group {
        one,
        many,
        expected,
    })
}

pub struct UnloadedLocation;

impl ProjectRule for UnloadedLocation {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_project(&self, assets: &[Asset], ctx: &RuleCtx<'_>, out: &mut Vec<Finding>) {
        // Library-row assets, one finding per folder and kind, at the first file.
        let mut folders: BTreeMap<(&str, AssetKind, bool), (usize, &Asset)> = BTreeMap::new();
        for asset in assets.iter().filter(|a| a.origin == Origin::LibraryRow) {
            let mut dir = parent(&asset.rel_path);
            let kiro = asset.kind == AssetKind::Hooks && asset.rel_path.ends_with(".kiro.hook");
            // Each hook config has its own folder (`hooks/<name>/hooks.json`): report the
            // collection once. Kiro hooks sit side by side.
            if asset.kind == AssetKind::Hooks && !kiro {
                dir = parent(dir);
            }
            let (n, first) = folders.entry((dir, asset.kind, kiro)).or_insert((0, asset));
            *n += 1;
            if asset.rel_path < first.rel_path {
                *first = asset;
            }
        }
        for ((dir, kind, kiro), (n, first)) in folders {
            let Some(g) = group(kind, kiro) else { continue };
            if in_plugin(dir, ctx.files) {
                continue;
            }
            let (noun, verb, them) = if n == 1 {
                (g.one, "is", "it")
            } else {
                (g.many, "are", "them")
            };
            out.push(Finding::new(
                &META,
                first,
                Span::new(0, 0),
                format!(
                    "{n} {noun} in `{}/` {verb} not loaded: move {them} to `{}`",
                    tail(display_dir(dir), QUOTED_PATH),
                    g.expected
                ),
            ));
        }

        for asset in assets {
            if asset.kind == AssetKind::Skill
                && asset.origin == Origin::Platform
                && let Some(expected) = skill_location(&asset.rel_path)
            {
                out.push(Finding::new(
                    &META,
                    asset,
                    Span::new(0, 0),
                    format!(
                        "Skill not loaded from `{}/`: expected `{}`",
                        tail(display_dir(parent(&asset.rel_path)), QUOTED_PATH),
                        tail(&expected, QUOTED_EXPECTED)
                    ),
                ));
            }
        }

        for m in ctx.misplaced {
            out.push(misplaced(m, ctx.root));
        }
    }
}

/// The folder of `rel_path`, `""` at the root.
fn parent(rel_path: &str) -> &str {
    rel_path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

fn display_dir(dir: &str) -> &str {
    if dir.is_empty() { "." } else { dir }
}

/// Whether `dir` or one of its ancestors holds a plugin manifest.
fn in_plugin(dir: &str, files: &HashSet<String>) -> bool {
    let mut dir = Some(dir);
    while let Some(d) = dir {
        let found = PLUGIN_MANIFESTS.iter().any(|m| {
            if d.is_empty() {
                files.contains(*m)
            } else {
                files.contains(&format!("{d}/{m}"))
            }
        });
        if found {
            return true;
        }
        dir = (!d.is_empty()).then(|| parent(d));
    }
    false
}

/// Where a `SKILL.md` not directly in `<skills-dir>/<name>/` belongs; `None` when it is
/// well placed or its layout says nothing (a repo that is one skill keeps it at the root).
fn skill_location(rel_path: &str) -> Option<String> {
    let dirs: Vec<&str> = rel_path.split('/').collect();
    let dirs = dirs.get(..dirs.len().saturating_sub(1))?;
    if dirs.len() >= 2 && dirs[dirs.len() - 2] == "skills" {
        return None;
    }
    if let Some(p) = dirs.iter().rposition(|d| SKILL_PLATFORMS.contains(d)) {
        return Some(format!("{}/skills/<name>/SKILL.md", dirs[..=p].join("/")));
    }
    let q = dirs.iter().rposition(|d| *d == "skills")?;
    Some(format!("{}/<name>/SKILL.md", dirs[..=q].join("/")))
}

fn misplaced(m: &Misplaced, root: &Path) -> Finding {
    let (dir, name) = m.rel_path.rsplit_once('/').unwrap_or(("", &m.rel_path));
    let name = tail(name, QUOTED_NAME);
    let message = match m.near_miss {
        NearMiss::Case(right) => {
            format!("`{name}` loads only on case-insensitive file systems: rename it to `{right}`")
        }
        NearMiss::Rename(right) => {
            format!("`{name}` is not loaded by any assistant: rename it to `{right}`")
        }
        NearMiss::Move(right) => format!(
            "Not loaded from `{}/`: move `{name}` to `{right}`",
            tail(display_dir(dir), QUOTED_PATH)
        ),
    };
    Finding {
        rule_id: META.id,
        severity: META.default_severity,
        location: Location {
            path: Arc::from(root.join(&m.rel_path)),
            line: 1,
            col: 1,
            end_line: 1,
            end_col: 1,
        },
        message,
        remediation: Some(META.remediation.into()),
        token_waste: 0,
        fingerprint: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_location_wants_skills_dir_name_file() {
        for (rel, want) in [
            (".claude/skills/review/SKILL.md", None),
            ("plugin/skills/review/SKILL.md", None),
            ("SKILL.md", None),
            ("review/SKILL.md", None),
            ("docs/review/SKILL.md", None),
            (
                ".claude/skills/SKILL.md",
                Some(".claude/skills/<name>/SKILL.md"),
            ),
            (
                ".claude/skill/review/SKILL.md",
                Some(".claude/skills/<name>/SKILL.md"),
            ),
            (
                "pkg/.github/skills/a/b/SKILL.md",
                Some("pkg/.github/skills/<name>/SKILL.md"),
            ),
            ("skills/a/b/SKILL.md", Some("skills/<name>/SKILL.md")),
        ] {
            assert_eq!(skill_location(rel).as_deref(), want, "{rel}");
        }
    }

    #[test]
    fn in_plugin_checks_every_ancestor() {
        let files: HashSet<String> = ["p/.claude-plugin/plugin.json", "q/plugin.json"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        assert!(in_plugin("p/agents", &files));
        assert!(in_plugin("q", &files));
        assert!(!in_plugin("agents", &files));
        assert!(!in_plugin("", &files));
    }
}
