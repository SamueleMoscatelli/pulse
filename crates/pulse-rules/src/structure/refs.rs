//! AI-STR-003: broken `@import`, relative link or skill path, and `@import` cycles.
//! Project scope: imports resolve against every walked file and the import graph.

use std::borrow::Cow;
use std::sync::LazyLock;

use pulse_core::refs::{is_file_import, resolve};
use pulse_core::{
    Asset, AssetKind, Category, Finding, KindSet, ProjectRule, RuleCtx, RuleMeta, Severity, Span,
};
use regex::Regex;

use crate::util::truncate;

pub static META: RuleMeta = RuleMeta {
    id: "AI-STR-003",
    name: "Broken reference or import",
    category: Category::Structure,
    default_severity: Severity::Medium,
    kinds: KindSet::MARKDOWN,
    summary: "An @import, link or skill path points at a file that does not exist.",
    remediation: "Fix the path or add the missing file; break import cycles.",
    inline_suppressible: true,
};

/// Chars of a reference quoted in a message.
const QUOTED: usize = 40;

/// Skill-relative resource paths: `scripts/x.py`, `./references/y.md`, `assets/z.png`.
#[allow(clippy::expect_used)] // static pattern (rust.md)
static SKILL_PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^|[^\w/.\-])(?:\./)?((?:scripts|references|assets)/[\w./\-]*\w)")
        .expect("static pattern")
});

pub struct BrokenReference;

impl ProjectRule for BrokenReference {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_project(&self, assets: &[Asset], ctx: &RuleCtx<'_>, out: &mut Vec<Finding>) {
        for asset in assets {
            if !META.kinds.contains(asset.kind) || asset.skipped.is_some() {
                continue;
            }
            imports(asset, ctx, out);
            links(asset, ctx, out);
            if asset.kind == AssetKind::Skill {
                skill_paths(asset, ctx, out);
            }
        }
    }
}

fn imports(asset: &Asset, ctx: &RuleCtx<'_>, out: &mut Vec<Finding>) {
    for import in &asset.markdown().imports {
        // AI-STR-003 FP guard: `@alice` is a mention, not a file.
        if !is_file_import(&import.path) || looks_like_example(&import.path) {
            continue;
        }
        let Some(target) = resolve(&asset.rel_path, &import.path) else {
            continue;
        };
        let quoted = truncate(&import.path, QUOTED);
        if !exists(ctx, &target) {
            out.push(Finding::new(
                &META,
                asset,
                import.span,
                format!("Broken import `@{quoted}`: no such file"),
            ));
        } else if ctx.imports.closes_cycle(&asset.rel_path, &target) {
            let mut f = Finding::new(
                &META,
                asset,
                import.span,
                format!("Import cycle: `@{quoted}` imports this file back"),
            );
            f.severity = Severity::High;
            out.push(f);
        }
    }
}

fn links(asset: &Asset, ctx: &RuleCtx<'_>, out: &mut Vec<Finding>) {
    for link in &asset.markdown().links {
        let Some(path) = link_path(&link.dest) else {
            continue;
        };
        if looks_like_example(&path) {
            continue;
        }
        let Some(target) = resolve(&asset.rel_path, &path) else {
            continue;
        };
        if exists(ctx, &target) {
            continue;
        }
        let span = asset
            .source
            .get(link.span.start as usize..link.span.end as usize)
            .and_then(|s| s.rfind(link.dest.as_str()))
            .and_then(|i| {
                let start = link.span.start.checked_add(u32::try_from(i).ok()?)?;
                let end = start.checked_add(u32::try_from(link.dest.len()).ok()?)?;
                Some(Span::new(start, end))
            })
            .unwrap_or(link.span);
        out.push(Finding::new(
            &META,
            asset,
            span,
            format!(
                "Broken link `{}`: no such file",
                truncate(&link.dest, QUOTED)
            ),
        ));
    }
}

/// The file part of a relative link destination; `None` for URLs and anchor-only links.
fn link_path(dest: &str) -> Option<Cow<'_, str>> {
    let dest = dest.trim();
    // A scheme (`https:`, `mailto:`) comes before any `/`; `//host` is protocol-relative.
    let scheme = dest
        .find(':')
        .is_some_and(|c| dest.find('/').is_none_or(|s| c < s));
    if dest.starts_with("//") || scheme {
        return None;
    }
    let path = dest.split(['#', '?']).next().unwrap_or("");
    if path.is_empty() {
        return None;
    }
    Some(percent_decode(path))
}

/// Decodes `%XX` escapes (`my%20file.md`); invalid escapes are kept as written.
fn percent_decode(s: &str) -> Cow<'_, str> {
    if !s.contains('%') {
        return s.into();
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(b)) => {
                out.push(b);
                i += 3;
            }
            (b, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).map_or(s.into(), Cow::Owned)
}

fn skill_paths(asset: &Asset, ctx: &RuleCtx<'_>, out: &mut Vec<Finding>) {
    let md = asset.markdown();
    let body_start = md.body.start as usize;
    let Some(body) = asset.source.get(body_start..) else {
        return;
    };
    for caps in SKILL_PATH.captures_iter(body) {
        let Some(m) = caps.get(1) else {
            continue;
        };
        let (Ok(start), Ok(end)) = (
            u32::try_from(body_start + m.start()),
            u32::try_from(body_start + m.end()),
        ) else {
            continue;
        };
        // Links are checked as links; examples (`scripts/your-tool.py`) are not real paths.
        if md
            .links
            .iter()
            .any(|l| l.span.start <= start && end <= l.span.end)
            || looks_like_example(m.as_str())
        {
            continue;
        }
        let Some(target) = resolve(&asset.rel_path, m.as_str()) else {
            continue;
        };
        // AI-STR-003 FP guard: project skills also name root-level scripts (`scripts/check.sh`).
        if !exists(ctx, &target) && !exists(ctx, m.as_str()) {
            out.push(Finding::new(
                &META,
                asset,
                Span::new(start, end),
                format!(
                    "Broken path `{}`: no such file in the skill",
                    truncate(m.as_str(), QUOTED)
                ),
            ));
        }
    }
}

/// AI-STR-003 FP guard: placeholder paths (`<name>.md`, `your-script.py`, `example/x`).
fn looks_like_example(path: &str) -> bool {
    path.contains('<') || path.contains("your-") || path.to_ascii_lowercase().contains("example")
}

/// Whether `rel` is a walked file or directory, or exists on disk (e.g. excluded from the scan).
fn exists(ctx: &RuleCtx<'_>, rel: &str) -> bool {
    if ctx.file_exists(rel) {
        return true;
    }
    let dir = format!("{rel}/");
    ctx.files.iter().any(|f| f.starts_with(&dir)) || ctx.root.join(rel).exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_path_skips_urls_and_anchors() {
        for url in [
            "https://x.dev/a.md",
            "mailto:a@b.c",
            "//cdn/x.js",
            "#setup",
            "",
        ] {
            assert_eq!(link_path(url), None, "{url}");
        }
        assert_eq!(link_path("docs/a.md#setup").as_deref(), Some("docs/a.md"));
        assert_eq!(link_path("a.md?raw=1").as_deref(), Some("a.md"));
        assert_eq!(link_path("my%20file.md").as_deref(), Some("my file.md"));
        assert_eq!(link_path("100%.md").as_deref(), Some("100%.md"));
    }

    #[test]
    fn skill_path_pattern_needs_a_path_boundary() {
        let paths = |s: &str| -> Vec<String> {
            SKILL_PATH
                .captures_iter(s)
                .filter_map(|c| Some(c.get(1)?.as_str().to_owned()))
                .collect()
        };
        assert_eq!(
            paths("Run `python scripts/fill.py`, see ./references/forms.md."),
            ["scripts/fill.py", "references/forms.md"]
        );
        assert!(paths("lib/scripts/x.py https://h/assets/a.png scripts/*.py").is_empty());
    }

    #[test]
    fn examples_are_recognized() {
        assert!(looks_like_example("scripts/<name>.py"));
        assert!(looks_like_example("references/your-doc.md"));
        assert!(looks_like_example("docs/Example.md"));
        assert!(!looks_like_example("scripts/fill.py"));
    }
}
