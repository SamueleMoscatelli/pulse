//! AI-STR-001: invalid or missing frontmatter; AI-STR-004: name / description constraints.

use pulse_core::parse::Frontmatter;
use pulse_core::text::levenshtein_within;
use pulse_core::{
    Asset, AssetKind, Category, Finding, KindSet, Rule, RuleCtx, RuleMeta, Severity, Span,
};

use crate::structure::schemas::{KIRO_STEERING, Required, Schema, schema_for};
use crate::util::truncate;

/// Chars of a key or name quoted in a message.
const QUOTED: usize = 40;
/// Largest edit distance offered as a typo hint (AI-STR-001).
const TYPO_DISTANCE: usize = 2;
/// AI-STR-004 limits.
const MAX_NAME: usize = 64;
const MAX_DESCRIPTION: usize = 1024;
/// Words a skill name may not contain (AI-STR-004).
const RESERVED: &[&str] = &["anthropic", "claude"];

pub static META_001: RuleMeta = RuleMeta {
    id: "AI-STR-001",
    name: "Invalid or missing frontmatter",
    category: Category::Structure,
    default_severity: Severity::High,
    kinds: KindSet::SKILL
        .union(KindSet::AGENT)
        .union(KindSet::PROMPT)
        .union(KindSet::SCOPED_INSTRUCTIONS),
    summary: "Frontmatter is unterminated, lacks a required key, or has an unknown key.",
    remediation: "Close the `---` block, add the required keys and fix misspelled ones.",
    inline_suppressible: true,
};

pub static META_004: RuleMeta = RuleMeta {
    id: "AI-STR-004",
    name: "Name / description constraint violated",
    category: Category::Structure,
    default_severity: Severity::Medium,
    kinds: KindSet::SKILL.union(KindSet::AGENT).union(KindSet::PROMPT),
    summary: "Skill name or description breaks the platform's format or length limits.",
    remediation: "Use a lowercase-hyphenated name matching the directory and a 1-1024 char description.",
    inline_suppressible: true,
};

pub struct InvalidFrontmatter;

impl Rule for InvalidFrontmatter {
    fn meta(&self) -> &'static RuleMeta {
        &META_001
    }

    fn check(&self, asset: &Asset, _ctx: &RuleCtx<'_>, out: &mut Vec<Finding>) {
        let Some(fm) = asset.markdown().frontmatter.as_ref() else {
            if opens_frontmatter(&asset.source) {
                out.push(Finding::new(
                    &META_001,
                    asset,
                    asset.lines.line_span(&asset.source, 1),
                    "Unterminated frontmatter: no closing \"---\" line",
                ));
            } else if asset.is_kiro_steering() {
                late_frontmatter(asset, out);
            } else if let Some(schema) = schema_for(asset)
                && schema.required != Required::None
            {
                out.push(Finding::new(
                    &META_001,
                    asset,
                    Span::default(),
                    format!(
                        "Missing frontmatter: {} needs {}",
                        schema.label,
                        required_list(schema.required)
                    ),
                ));
            }
            return;
        };
        // Invalid YAML is reported by AI-STR-002 (ADR-025).
        let (Some(schema), Ok(map)) = (schema_for(asset), fm.value.as_ref()) else {
            return;
        };
        let present = |k: &&str| fm.get(k).is_some_and(|v| !v.is_null());
        let opener = asset
            .lines
            .line_span(&asset.source, asset.lines.line_of(fm.span.start));
        match schema.required {
            Required::None => {}
            Required::All(keys) => {
                for key in keys.iter().filter(|k| !present(k)) {
                    out.push(Finding::new(
                        &META_001,
                        asset,
                        opener,
                        format!("Missing frontmatter key \"{key}\" ({})", schema.label),
                    ));
                }
            }
            Required::AnyOf(keys) => {
                if !keys.iter().any(present) {
                    out.push(Finding::new(
                        &META_001,
                        asset,
                        opener,
                        format!(
                            "Missing frontmatter key: {} needs {}",
                            schema.label,
                            required_list(schema.required)
                        ),
                    ));
                }
            }
        }
        if std::ptr::eq(schema, &KIRO_STEERING) {
            kiro_inclusion(asset, fm, opener, out);
        }
        for key in map.keys().filter_map(|k| k.as_str()) {
            if schema.is_known(key) {
                continue;
            }
            let mut f = Finding::new(
                &META_001,
                asset,
                key_span(asset, fm, key),
                unknown_key_message(schema, key),
            );
            f.severity = Severity::Low;
            out.push(f);
        }
    }
}

/// Kiro steering `inclusion` modes (https://kiro.dev/docs/steering/).
const KIRO_MODES: &[&str] = &["always", "fileMatch", "manual", "auto"];

/// Kiro steering: `inclusion` is one of [`KIRO_MODES`]; `fileMatch` needs a string or
/// string-list `fileMatchPattern`; `auto` needs `name` and `description`.
fn kiro_inclusion(asset: &Asset, fm: &Frontmatter, opener: Span, out: &mut Vec<Finding>) {
    let Some(value) = fm.get("inclusion").filter(|v| !v.is_null()) else {
        return;
    };
    let mode = value.as_str().map(str::trim);
    let mut push = |span: Span, msg: String| out.push(Finding::new(&META_001, asset, span, msg));
    match mode {
        Some("fileMatch") => {
            let glob = |s: Option<&str>| s.is_some_and(|s| !s.trim().is_empty());
            match fm.get("fileMatchPattern").filter(|v| !v.is_null()) {
                None => push(
                    opener,
                    "Missing frontmatter key \"fileMatchPattern\" (inclusion: fileMatch)".into(),
                ),
                Some(p)
                    if !glob(p.as_str())
                        && !p.as_sequence().is_some_and(|l| {
                            !l.is_empty() && l.iter().all(|g| glob(g.as_str()))
                        }) =>
                {
                    push(
                        key_span(asset, fm, "fileMatchPattern"),
                        "Invalid fileMatchPattern: use a glob string or a list of globs".into(),
                    );
                }
                Some(_) => {}
            }
        }
        Some("auto") => {
            for key in ["name", "description"] {
                if fm.get(key).is_none_or(|v| v.is_null()) {
                    push(
                        opener,
                        format!("Missing frontmatter key \"{key}\" (inclusion: auto)"),
                    );
                }
            }
        }
        Some("always" | "manual") => {}
        _ => {
            let hint = mode
                .and_then(|m| {
                    KIRO_MODES
                        .iter()
                        .filter_map(|k| Some((levenshtein_within(m, k, TYPO_DISTANCE)?, *k)))
                        .min_by_key(|&(d, _)| d)
                })
                .map_or_else(
                    || "use always, fileMatch, manual or auto".to_owned(),
                    |(_, k)| format!("did you mean \"{k}\"?"),
                );
            let written =
                mode.map_or_else(String::new, |m| format!(" \"{}\"", truncate(m, QUOTED)));
            push(
                key_span(asset, fm, "inclusion"),
                format!("Invalid inclusion{written}: {hint}"),
            );
        }
    }
}

/// Kiro reads frontmatter only on the first line: a later `---` block (outside code
/// blocks) that sets `inclusion` is ignored, so the file loads as `always`.
fn late_frontmatter(asset: &Asset, out: &mut Vec<Finding>) {
    let mut code = asset.markdown().code_blocks.iter().peekable();
    // The open `---` line, and whether the block so far sets `inclusion`.
    let mut open: Option<(u32, bool)> = None;
    for (i, line) in asset.source.lines().enumerate() {
        let n = u32::try_from(i + 1).unwrap_or(u32::MAX);
        while code.next_if(|b| b.lines.1 < n).is_some() {}
        if code.peek().is_some_and(|b| b.lines.0 <= n) {
            open = None;
            continue;
        }
        let line = line.trim_end();
        match open {
            _ if line == "---" => {
                if let Some((start, true)) = open {
                    out.push(Finding::new(
                        &META_001,
                        asset,
                        asset.lines.line_span(&asset.source, start),
                        "Frontmatter not on the first line: Kiro ignores it and always includes the file",
                    ));
                    return;
                }
                open = Some((n, false));
            }
            Some((start, inclusion)) => {
                // AI-STR-001 FP guard: every line between two thematic breaks must look
                // like YAML (`key: value`, a `- item` or an indented continuation).
                open = yaml_like(line)
                    .then(|| (start, inclusion || yaml_key(line) == Some("inclusion")));
            }
            None => {}
        }
    }
}

/// The key of a top-level `key: value` line.
fn yaml_key(line: &str) -> Option<&str> {
    let (key, _) = line.split_once(':')?;
    (!key.is_empty()
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'))
    .then_some(key)
}

fn yaml_like(line: &str) -> bool {
    line.is_empty() || line.starts_with([' ', '\t', '-', '#']) || yaml_key(line).is_some()
}

/// Whether `source` starts a frontmatter block (optional BOM, then a `---` line).
fn opens_frontmatter(source: &str) -> bool {
    let s = source.strip_prefix('\u{feff}').unwrap_or(source);
    s.starts_with("---\n") || s.starts_with("---\r\n")
}

fn required_list(required: Required) -> String {
    match required {
        Required::None => String::new(),
        Required::All(keys) => keys.join(", "),
        Required::AnyOf(keys) => match keys.split_last() {
            Some((last, rest)) if !rest.is_empty() => format!("{} or {last}", rest.join(", ")),
            _ => keys.join(""),
        },
    }
}

/// The closest known key within [`TYPO_DISTANCE`], first in table order on ties.
fn suggestion(schema: &Schema, key: &str) -> Option<&'static str> {
    schema
        .known
        .iter()
        .filter_map(|k| Some((levenshtein_within(key, k, TYPO_DISTANCE)?, *k)))
        .min_by_key(|&(d, _)| d)
        .map(|(_, k)| k)
}

fn unknown_key_message(schema: &Schema, key: &str) -> String {
    let quoted = truncate(key, QUOTED);
    match suggestion(schema, key) {
        Some(hint) => format!("Unknown frontmatter key \"{quoted}\" (did you mean \"{hint}\"?)"),
        None => format!("Unknown frontmatter key \"{quoted}\" for {}", schema.label),
    }
}

/// The line of top-level `key`, else the opening `---` line.
pub(crate) fn key_span(asset: &Asset, fm: &Frontmatter, key: &str) -> Span {
    let line = fm
        .key_lines
        .get(key)
        .copied()
        .unwrap_or_else(|| asset.lines.line_of(fm.span.start));
    asset.lines.line_span(&asset.source, line)
}

pub struct NameDescription;

impl Rule for NameDescription {
    fn meta(&self) -> &'static RuleMeta {
        &META_004
    }

    fn check(&self, asset: &Asset, _ctx: &RuleCtx<'_>, out: &mut Vec<Finding>) {
        let Some(fm) = asset.markdown().frontmatter.as_ref() else {
            return;
        };
        let mut push = |key: &str, msg: String| {
            out.push(Finding::new(
                &META_004,
                asset,
                key_span(asset, fm, key),
                msg,
            ));
        };
        if let Some(desc) = fm.get("description").and_then(|v| v.as_str()) {
            let len = desc.trim().chars().count();
            if len == 0 {
                push("description", "Empty description".to_owned());
            } else if len > MAX_DESCRIPTION {
                push(
                    "description",
                    format!("Description too long: {len} chars (max {MAX_DESCRIPTION})"),
                );
            }
        }
        if asset.kind != AssetKind::Skill {
            return;
        }
        let Some(name) = fm.get("name").and_then(|v| v.as_str()) else {
            return;
        };
        let quoted = truncate(name, QUOTED);
        if !is_kebab(name) {
            push(
                "name",
                format!("Invalid skill name \"{quoted}\": use lowercase letters, digits, hyphens"),
            );
        }
        let len = name.chars().count();
        if len > MAX_NAME {
            push(
                "name",
                format!("Skill name too long: {len} chars (max {MAX_NAME})"),
            );
        }
        let lower = name.to_ascii_lowercase();
        if let Some(word) = RESERVED.iter().find(|w| lower.contains(*w)) {
            push(
                "name",
                format!("Skill name \"{quoted}\" contains reserved word \"{word}\""),
            );
        }
        if let Some(dir) = skill_dir(&asset.rel_path)
            && dir != name
        {
            push(
                "name",
                format!(
                    "Skill name \"{quoted}\" differs from its directory \"{}\"",
                    truncate(dir, QUOTED)
                ),
            );
        }
    }
}

/// `^[a-z0-9]+(-[a-z0-9]+)*$`
fn is_kebab(name: &str) -> bool {
    !name.is_empty()
        && name.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}

/// The directory holding a `SKILL.md`, if it is not at the scan root.
fn skill_dir(rel_path: &str) -> Option<&str> {
    let parent = rel_path.strip_suffix("SKILL.md")?.strip_suffix('/')?;
    parent.rsplit('/').next().filter(|d| !d.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kebab_names() {
        assert!(is_kebab("pdf-tools"));
        assert!(is_kebab("a1-b2-3"));
        for bad in ["", "-a", "a-", "a--b", "PDF", "pdf_tools", "pdf tools", "é"] {
            assert!(!is_kebab(bad), "{bad}");
        }
    }

    #[test]
    fn skill_dir_from_path() {
        assert_eq!(skill_dir(".claude/skills/pdf/SKILL.md"), Some("pdf"));
        assert_eq!(skill_dir("pdf/SKILL.md"), Some("pdf"));
        assert_eq!(skill_dir("SKILL.md"), None);
        assert_eq!(skill_dir("fail_SKILL.md"), None);
    }

    #[test]
    fn opener_and_required_list() {
        assert!(opens_frontmatter("---\nname: x\n"));
        assert!(opens_frontmatter("\u{feff}---\r\n"));
        assert!(!opens_frontmatter("--- \n"));
        assert!(!opens_frontmatter("# T\n---\n"));
        assert_eq!(
            required_list(Required::AnyOf(&["a", "b", "c"])),
            "a, b or c"
        );
        assert_eq!(required_list(Required::All(&["a", "b"])), "a, b");
    }
}
