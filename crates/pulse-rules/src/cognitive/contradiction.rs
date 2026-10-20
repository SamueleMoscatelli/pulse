//! Contradictory directives: within one file (AI-COG-001) and across files that load
//! together (AI-COG-005).

use std::collections::{HashMap, HashSet};

use pulse_core::{
    Asset, AssetKind, Category, ConfigError, Finding, KindSet, LoadMode, ProjectRule, Rule,
    RuleCtx, RuleMeta, Severity, Span,
};
use serde::Deserialize;

use super::directives::{self, Directive, Polarity, contains_phrase, is, lemma, same, words};
use crate::lexicon::directives::{EXCLUSIVE_SETS, GENERIC_HEADINGS, STOP};
use crate::util::{tail, truncate};

pub static META: RuleMeta = RuleMeta {
    id: "AI-COG-001",
    name: "Contradictory directives",
    category: Category::Cognitive,
    default_severity: Severity::Medium,
    kinds: KindSet::MARKDOWN,
    summary: "Two directives in the file contradict each other.",
    remediation: "Remove one directive, or scope both (\"for tests…\", or under headings naming each scope).",
    inline_suppressible: true,
};

/// Longest directive quoted in a finding (two quotes fit in 100 chars).
const MAX_QUOTE: usize = 34;

/// `[rules.AI-COG-001]` options.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Options {
    /// Extra sets of mutually exclusive choices, e.g. `[["bun", "deno"]]`.
    pub exclusive: Vec<Vec<String>>,
}

pub struct ContradictoryDirectives;

impl Rule for ContradictoryDirectives {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn validate(&self, ctx: &RuleCtx<'_>) -> Result<(), ConfigError> {
        ctx.options::<Options>(META.id).map(drop)
    }

    fn check(&self, asset: &Asset, ctx: &RuleCtx<'_>, out: &mut Vec<Finding>) {
        let ds = directives::extract(asset);
        if ds.len() < 2 {
            return;
        }
        let opts = ctx.options::<Options>(META.id).unwrap_or_default();
        let ps: Vec<_> = ds
            .iter()
            .map(|d| Prepared::new(d, &opts.exclusive, false))
            .collect();
        for (j, b) in ps.iter().enumerate().skip(1) {
            let Some(a) = ps[..j].iter().find(|a| conflict(a, b)) else {
                continue;
            };
            let (a, b) = (a.directive, b.directive);
            let line = asset.span_to_location(a.span).line;
            let msg = format!(
                "Contradicts line {line}: \"{}\" vs \"{}\"",
                quote(asset, a.span, MAX_QUOTE),
                quote(asset, b.span, MAX_QUOTE)
            );
            out.push(Finding::new(&META, asset, b.span, msg));
        }
    }
}

/// A directive with the per-directive facts [`conflict`] needs, computed once so the
/// pairwise loop stays cheap.
pub struct Prepared<'d, 'a> {
    pub directive: &'d Directive<'a>,
    /// The section heading names a scope (`Python`, not `Code style`).
    scoped_section: bool,
    /// `(set, member)` of every exclusive-set member the object names (positive only);
    /// built-in sets first, then `extra` sets offset by `EXCLUSIVE_SETS.len()`.
    members: Vec<(usize, usize)>,
}

impl<'d, 'a> Prepared<'d, 'a> {
    /// Prepares `directive`, with `extra` exclusive sets from the options. Across files
    /// (`cross_file`), a document title is not a section scope: every file has its own.
    pub fn new(directive: &'d Directive<'a>, extra: &[Vec<String>], cross_file: bool) -> Self {
        let mut members = Vec::new();
        if directive.polarity == Polarity::Positive {
            let obj = &directive.object;
            for (si, set) in EXCLUSIVE_SETS.iter().enumerate() {
                for (mi, m) in set.iter().enumerate() {
                    if contains_phrase(obj, m) {
                        members.push((si, mi));
                    }
                }
            }
            for (si, set) in extra.iter().enumerate() {
                for (mi, m) in set.iter().enumerate() {
                    if contains_phrase(obj, m) {
                        members.push((EXCLUSIVE_SETS.len() + si, mi));
                    }
                }
            }
        }
        Self {
            directive,
            scoped_section: !(cross_file && directive.titled)
                && directive.section.is_some_and(names_scope),
            members,
        }
    }
}

/// Whether `a` and `b` contradict each other (spec AI-COG-001 → Detect 2–3, FP guards).
pub fn conflict(pa: &Prepared<'_, '_>, pb: &Prepared<'_, '_>) -> bool {
    let (a, b) = (pa.directive, pb.directive);
    let hit = match (a.polarity, b.polarity) {
        (Polarity::Positive, Polarity::Positive) => exclusive(&pa.members, &pb.members),
        (Polarity::Negative, Polarity::Negative) => false,
        (Polarity::Positive, Polarity::Negative) => {
            same(a.verb, b.verb) && narrows(&a.object, &b.object)
        }
        (Polarity::Negative, Polarity::Positive) => {
            same(a.verb, b.verb) && narrows(&b.object, &a.object)
        }
    };
    // AI-COG-001 FP guards: scoped qualifiers with different contexts, different scoped sections.
    let scoped_apart = pa.scoped_section
        && pb.scoped_section
        && !a
            .section
            .unwrap_or("")
            .eq_ignore_ascii_case(b.section.unwrap_or(""));
    hit && same_words(&a.context, &b.context) && !scoped_apart
}

/// Whether `a` names one member of a set and `b` another, and neither names the other's.
fn exclusive(a: &[(usize, usize)], b: &[(usize, usize)]) -> bool {
    a.iter().any(|&(sa, x)| {
        b.iter()
            .any(|&(sb, y)| sa == sb && x != y && !a.contains(&(sa, y)) && !b.contains(&(sb, x)))
    })
}

/// Whether the negative object `neg` is a prefix of the positive one `pos`: "Never use
/// semicolons" denies "Always use semicolons at line end", but "Never use TypeScript enums"
/// only narrows "Always use TypeScript".
fn narrows(pos: &[&str], neg: &[&str]) -> bool {
    !neg.is_empty() && neg.len() <= pos.len() && same_words(&pos[..neg.len()], neg)
}

fn same_words(a: &[&str], b: &[&str]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| same(x, y))
}

/// Whether a heading names a scope: some word is not generic (`Code style`, `Rules`).
fn names_scope(heading: &str) -> bool {
    let mut ws = Vec::new();
    words(heading, &mut ws);
    ws.iter().any(|w| {
        let l = lemma(w.text);
        !is(l, GENERIC_HEADINGS) && !is(l, STOP) && !is(w.text, GENERIC_HEADINGS)
    })
}

/// The directive's text on one line, without trailing punctuation, truncated.
fn quote(asset: &Asset, span: Span, max: usize) -> String {
    let raw = asset
        .source
        .get(span.start as usize..span.end as usize)
        .unwrap_or("");
    let text = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    let text = text.trim_end_matches(['.', '!', '?', ';', ':', ',']);
    truncate(text, max).into_owned()
}

pub static META_CROSS: RuleMeta = RuleMeta {
    id: "AI-COG-005",
    name: "Cross-file contradiction",
    category: Category::Cognitive,
    default_severity: Severity::Medium,
    kinds: KindSet::INSTRUCTIONS.union(KindSet::SCOPED_INSTRUCTIONS),
    summary: "Two instruction files that load together contradict each other.",
    remediation: "Keep the directive in one file, or scope the nested one (\"in this package…\").",
    inline_suppressible: true,
};

/// Longest directive quoted in an AI-COG-005 finding (the message also names a path).
const MAX_CROSS_QUOTE: usize = 26;
/// Chars of the other file's path quoted in an AI-COG-005 message (its tail is kept).
const QUOTED_PATH: usize = 20;

pub struct CrossFileContradiction;

impl ProjectRule for CrossFileContradiction {
    fn meta(&self) -> &'static RuleMeta {
        &META_CROSS
    }

    fn check_project(&self, assets: &[Asset], ctx: &RuleCtx<'_>, out: &mut Vec<Finding>) {
        let groups = load_groups(assets, ctx);
        if groups.is_empty() {
            return;
        }
        // Every file in some group, extracted and prepared once.
        let mut files: Vec<&Asset> = Vec::new();
        let mut index: HashMap<&str, usize> = HashMap::new();
        for g in &groups {
            for &a in &g.files {
                index.entry(a.rel_path.as_str()).or_insert_with(|| {
                    files.push(a);
                    files.len() - 1
                });
            }
        }
        // The exclusive sets are AI-COG-001's (one logic, one option).
        let opts = ctx.options::<Options>(META.id).unwrap_or_default();
        let ds: Vec<Vec<Directive<'_>>> = files.iter().map(|a| directives::extract(a)).collect();
        let ps: Vec<Vec<Prepared<'_, '_>>> = ds
            .iter()
            .map(|d| {
                d.iter()
                    .map(|d| Prepared::new(d, &opts.exclusive, true))
                    .collect()
            })
            .collect();
        let mut reported: HashSet<(usize, u32)> = HashSet::new();
        for g in &groups {
            let ids: Vec<usize> = g.files.iter().map(|a| index[a.rel_path.as_str()]).collect();
            for (pos, &fb) in ids.iter().enumerate().skip(g.first_new.max(1)) {
                for b in &ps[fb] {
                    let found = ids[..pos]
                        .iter()
                        .find_map(|&fa| Some((fa, ps[fa].iter().find(|a| conflict(a, b))?)));
                    let Some((fa, a)) = found else {
                        continue;
                    };
                    if !reported.insert((fb, b.directive.span.start)) {
                        continue;
                    }
                    let (asset_a, asset_b) = (files[fa], files[fb]);
                    let (a, b) = (a.directive, b.directive);
                    let line = asset_a.span_to_location(a.span).line;
                    let msg = format!(
                        "Contradicts {}:{line}: \"{}\" vs \"{}\"",
                        tail(&asset_a.rel_path, QUOTED_PATH),
                        quote(asset_a, a.span, MAX_CROSS_QUOTE),
                        quote(asset_b, b.span, MAX_CROSS_QUOTE)
                    );
                    out.push(Finding::new(&META_CROSS, asset_b, b.span, msg));
                }
            }
        }
    }
}

/// Files loaded into the same session, in load order. Only pairs whose later file is at
/// `first_new` or after are checked, so pairs shared with an earlier group aren't repeated.
struct LoadGroup<'a> {
    files: Vec<&'a Asset>,
    first_new: usize,
}

/// The load-together groups (spec AI-COG-005): the always-on set (root Always instructions,
/// alwaysApply rules and their `@imports`), then one group per nested `CLAUDE.md`/`AGENTS.md`
/// with the always-on set, its nested ancestors and their imports.
fn load_groups<'a>(assets: &'a [Asset], ctx: &RuleCtx<'a>) -> Vec<LoadGroup<'a>> {
    let by_path: HashMap<&str, &Asset> = assets.iter().map(|a| (a.rel_path.as_str(), a)).collect();
    let instr = |a: &Asset| {
        matches!(
            a.kind,
            AssetKind::Instructions | AssetKind::ScopedInstructions
        ) && a.skipped.is_none()
    };
    let mut roots: Vec<&Asset> = assets
        .iter()
        .filter(|a| instr(a) && a.effective_load() == LoadMode::Always)
        .collect();
    roots.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    let mut nested: Vec<&Asset> = assets
        .iter()
        .filter(|a| {
            a.kind == AssetKind::Instructions
                && a.skipped.is_none()
                && a.load == LoadMode::Conditional
                && matches!(file_name(&a.rel_path), "CLAUDE.md" | "AGENTS.md")
        })
        .collect();
    nested.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));

    // Appends `a` and the files it imports, each once.
    let push = |files: &mut Vec<&'a Asset>, a: &'a Asset| {
        if files.iter().any(|f| f.rel_path == a.rel_path) {
            return;
        }
        files.push(a);
        for rel in ctx.imports.reachable(&a.rel_path).into_keys() {
            let Some(f) = by_path.get(rel).copied().or_else(|| ctx.imports.file(rel)) else {
                continue;
            };
            if !files.iter().any(|x| x.rel_path == f.rel_path) {
                files.push(f);
            }
        }
    };
    let mut base = Vec::new();
    for r in roots {
        push(&mut base, r);
    }
    let mut groups = Vec::new();
    for leaf in &nested {
        let dir = parent(&leaf.rel_path);
        let mut ancestors: Vec<&Asset> = nested
            .iter()
            .copied()
            .filter(|a| {
                let d = parent(&a.rel_path);
                d.len() < dir.len() && dir.starts_with(d) && dir.as_bytes()[d.len()] == b'/'
            })
            .collect();
        ancestors.sort_by_key(|a| (parent(&a.rel_path).len(), a.rel_path.as_str()));
        let mut files = base.clone();
        for a in ancestors {
            push(&mut files, a);
        }
        let first_new = files.len();
        push(&mut files, leaf);
        if first_new > 0 && files.len() > first_new {
            groups.push(LoadGroup { files, first_new });
        }
    }
    if base.len() > 1 {
        groups.insert(
            0,
            LoadGroup {
                files: base,
                first_new: 1,
            },
        );
    }
    groups
}

fn file_name(rel: &str) -> &str {
    rel.rsplit('/').next().unwrap_or(rel)
}

/// Directory of `rel` (`""` at the root), without a trailing `/`.
fn parent(rel: &str) -> &str {
    rel.rsplit_once('/').map_or("", |(d, _)| d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parent_dirs() {
        assert_eq!(parent("CLAUDE.md"), "");
        assert_eq!(parent("packages/x/CLAUDE.md"), "packages/x");
        assert_eq!(file_name("packages/x/AGENTS.md"), "AGENTS.md");
    }

    #[test]
    fn scope_headings() {
        assert!(names_scope("Python"));
        assert!(names_scope("Frontend rules"));
        assert!(!names_scope("Code style"));
        assert!(!names_scope("General Rules"));
        assert!(!names_scope("Do and Don't"));
    }

    #[test]
    fn exclusive_needs_distinct_members() {
        // Set 0 = {npm, yarn, pnpm}, set 1 = {bun, deno}.
        assert!(exclusive(&[(0, 0)], &[(0, 2)]));
        assert!(!exclusive(&[(0, 0)], &[(0, 0)]));
        assert!(!exclusive(&[(0, 0), (0, 1)], &[(0, 1)]));
        assert!(!exclusive(&[(1, 0)], &[(0, 1)]));
    }

    #[test]
    fn negative_object_must_not_be_more_specific() {
        assert!(narrows(&["semicolon", "end"], &["Semicolon"]));
        assert!(narrows(&["semicolon"], &["semicolon"]));
        assert!(!narrows(&["typescript"], &["typescript", "enum"]));
        assert!(!narrows(&["typescript", "enum"], &["typescript", "strict"]));
    }
}
