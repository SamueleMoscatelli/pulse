//! Inline (`<!-- pulse-ignore … -->`) and config (`[[ignore]]`) suppressions (spec
//! 05-cli-config → Inline suppressions).

use std::collections::HashMap;
use std::path::Path;

use globset::{GlobBuilder, GlobMatcher};
use pulse_core::config::{CONFIG_FILE, did_you_mean};
use pulse_core::{Asset, Config, ConfigError, Finding, KindSet, Severity, Span};
use pulse_rules::{all_meta, structure};

const NOTE_REMEDIATION: &str =
    "Remove the comment, or name the rule IDs it should suppress (see `pulse explain`).";

/// Compiled `[[ignore]]` entries.
pub struct Ignores(Vec<(GlobMatcher, Vec<String>)>);

impl Ignores {
    /// Compiles the globs and checks every listed rule ID against the registry.
    pub fn new(config: &Config) -> Result<Self, ConfigError> {
        let invalid = |message: String| ConfigError::Invalid {
            path: CONFIG_FILE.into(),
            message,
        };
        let mut entries = Vec::with_capacity(config.ignore.len());
        for entry in &config.ignore {
            let glob = GlobBuilder::new(&entry.path)
                .literal_separator(true)
                .build()
                .map_err(|e| invalid(format!("invalid glob in [[ignore]] path: {e}")))?;
            for id in &entry.rules {
                let known = pulse_rules::find(id).is_some()
                    || config.custom_rules.iter().any(|c| &c.id == id);
                if !known {
                    let ids = all_meta()
                        .map(|m| m.id)
                        .chain(config.custom_rules.iter().map(|c| c.id.as_str()));
                    let hint = did_you_mean(id, ids)
                        .map_or_else(String::new, |s| format!(" (did you mean `{s}`?)"));
                    return Err(invalid(format!("unknown rule `{id}` in [[ignore]]{hint}")));
                }
            }
            entries.push((glob.compile_matcher(), entry.rules.clone()));
        }
        Ok(Self(entries))
    }

    fn matches(&self, rel: &str, rule_id: &str) -> bool {
        self.0.iter().any(|(glob, rules)| {
            (rules.is_empty() || rules.iter().any(|r| r == rule_id)) && glob.is_match(rel)
        })
    }
}

/// How a suppression applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    /// Findings starting on this 1-based line.
    Line(u32),
    /// The whole file.
    File,
    /// Nothing (no target line, or a misplaced `pulse-ignore-file`).
    None,
}

/// One parsed suppression comment.
#[derive(Debug)]
struct Directive {
    span: Span,
    scope: Scope,
    /// Rule IDs with a "suppressed something" flag.
    ids: Vec<(String, bool)>,
    /// Set when the comment is unusable; reported instead of per-ID notes.
    invalid: Option<&'static str>,
}

/// Applies inline suppressions in `assets`, then `ignores`. Returns the kept findings, plus
/// Info notes (AI-STR-002) about suppression comments: unused ones, ones naming a
/// non-inline-suppressible rule (ADR-006), and malformed ones; and the number dropped. `ran` tells
/// whether a rule ran, so a suppression for a disabled rule is not reported as unused.
pub fn apply(
    findings: Vec<Finding>,
    assets: &[&Asset],
    root: &Path,
    ignores: &Ignores,
    ran: impl Fn(&str) -> bool,
) -> (Vec<Finding>, usize) {
    let mut directives: HashMap<&Path, (&Asset, Vec<Directive>)> = assets
        .iter()
        .filter_map(|a| {
            let d = parse(a);
            (!d.is_empty()).then(|| (&*a.path, (*a, d)))
        })
        .collect();

    let before = findings.len();
    let mut kept: Vec<Finding> = findings
        .into_iter()
        .filter(|f| match directives.get_mut(&*f.location.path) {
            Some((_, ds)) => !suppress(ds, f),
            None => true,
        })
        .collect();
    let mut suppressed = before - kept.len();

    // Notes are pushed in map order; the pipeline sorts afterwards.
    // Notes are AI-STR-002 findings: none when that rule is off.
    let notes_on = ran(structure::content::META.id);
    for (asset, ds) in directives.values().filter(|_| notes_on) {
        for d in ds {
            notes(asset, d, &ran, &mut kept);
        }
    }

    if ignores.0.is_empty() {
        return (kept, suppressed);
    }
    let total = kept.len();
    kept.retain(|f| {
        let rel = f
            .location
            .path
            .strip_prefix(root)
            .unwrap_or(&f.location.path);
        !ignores.matches(&rel.to_string_lossy().replace('\\', "/"), f.rule_id)
    });
    suppressed += total - kept.len();
    (kept, suppressed)
}

/// Whether a directive in `ds` suppresses `f`; marks the matching ID as used.
fn suppress(ds: &mut [Directive], f: &Finding) -> bool {
    let inline_ok = pulse_rules::find(f.rule_id).is_none_or(|m| m.inline_suppressible);
    if !inline_ok {
        return false;
    }
    let mut hit = false;
    for d in ds.iter_mut().filter(|d| d.invalid.is_none()) {
        let applies = match d.scope {
            Scope::File => true,
            Scope::Line(l) => l == f.location.line,
            Scope::None => false,
        };
        if applies {
            for (id, used) in &mut d.ids {
                if id == f.rule_id {
                    *used = true;
                    hit = true;
                }
            }
        }
    }
    hit
}

/// Pushes the Info notes for one directive.
fn notes(asset: &Asset, d: &Directive, ran: &impl Fn(&str) -> bool, out: &mut Vec<Finding>) {
    let mut note = |msg: String| {
        let mut f = Finding::new(&structure::content::META, asset, d.span, msg);
        f.severity = Severity::Info;
        f.remediation = Some(NOTE_REMEDIATION);
        out.push(f);
    };
    if let Some(reason) = d.invalid {
        note(format!("invalid suppression: {reason}"));
        return;
    }
    for (id, used) in &d.ids {
        let meta = pulse_rules::find(id);
        if meta.is_none() && !id.starts_with("X-") {
            note(format!("invalid suppression: unknown rule {id}"));
        } else if meta.is_some_and(|m| !m.inline_suppressible) {
            note(format!(
                "suppression ignored: {id} can only be suppressed in pulse.toml"
            ));
        } else if !used && ran(id) {
            note(format!("unused suppression: {id}"));
        }
    }
}

/// Parses the suppression comments of a Markdown asset (JSON assets have none).
fn parse(asset: &Asset) -> Vec<Directive> {
    if !KindSet::MARKDOWN.contains(asset.kind)
        || asset.skipped.is_some()
        || !asset.source.contains("pulse-ignore")
    {
        return Vec::new();
    }
    let md = asset.markdown();
    let src = &*asset.source;
    let text = |s: Span| src.get(s.start as usize..s.end as usize).unwrap_or("");
    let first_heading = md.headings.first().map_or(u32::MAX, |h| h.span.start);

    let mut out = Vec::new();
    // 1-based lines holding nothing but a suppression comment (skipped by next-line targets).
    let mut standalone = Vec::new();
    for &span in &md.html_comments {
        let Some((file, ids)) = directive(text(span)) else {
            continue;
        };
        let line = asset.lines.line_of(span.start);
        let line_span = asset.lines.line_span(src, line);
        let before = src
            .get(line_span.start as usize..span.start as usize)
            .unwrap_or("");
        let trailing = !before.trim().is_empty();
        let end_line = asset
            .lines
            .line_of(span.end.saturating_sub(1).max(span.start));
        if !trailing && end_line == line && text(line_span).trim() == text(span) {
            standalone.push(line);
        }
        let mut invalid = None;
        let scope = if file {
            if span.start < first_heading {
                Scope::File
            } else {
                invalid = Some("pulse-ignore-file must appear before the first heading");
                Scope::None
            }
        } else if trailing {
            Scope::Line(line)
        } else {
            // Resolved below, once every standalone line is known.
            Scope::Line(end_line)
        };
        if ids.is_empty() {
            invalid = Some("no rule IDs listed");
        } else if ids.iter().any(|id| id.eq_ignore_ascii_case("all")) {
            invalid = Some("`all` is not accepted; list the rule IDs");
        }
        out.push((
            trailing,
            Directive {
                span,
                scope,
                ids: ids.into_iter().map(|id| (id.to_owned(), false)).collect(),
                invalid,
            },
        ));
    }

    // `next[l]`: first line >= l that a next-line comment can target. One reverse pass keeps
    // stacked comments and long blank runs linear (assets are untrusted).
    let line_count = u32::try_from(asset.lines.line_count()).unwrap_or(u32::MAX);
    let mut next = vec![None; line_count as usize + 2];
    for l in (1..=line_count).rev() {
        let content = standalone.binary_search(&l).is_err()
            && !text(asset.lines.line_span(src, l)).trim().is_empty();
        next[l as usize] = if content {
            Some(l)
        } else {
            next[l as usize + 1]
        };
    }
    out.into_iter()
        .map(|(trailing, mut d)| {
            if let (false, Scope::Line(end)) = (trailing, d.scope) {
                d.scope = next
                    .get(end as usize + 1)
                    .copied()
                    .flatten()
                    .map_or(Scope::None, Scope::Line);
            }
            d
        })
        .collect()
}

/// Parses `<!-- pulse-ignore[-file] ID[, ID…] [-- reason] -->` into (file scope, IDs).
fn directive(comment: &str) -> Option<(bool, Vec<&str>)> {
    let inner = comment.strip_prefix("<!--")?;
    let inner = inner.strip_suffix("-->").unwrap_or(inner).trim();
    let (file, rest) = match inner.strip_prefix("pulse-ignore-file") {
        Some(rest) => (true, rest),
        None => (false, inner.strip_prefix("pulse-ignore")?),
    };
    if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let ids = rest.split_once("--").map_or(rest, |(ids, _reason)| ids);
    let ids = ids
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .collect();
    Some((file, ids))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use pulse_core::{AssetKind, LoadMode};

    fn asset(src: &str) -> Asset {
        Asset::new(
            PathBuf::from("/r/docs/CLAUDE.md"),
            "docs/CLAUDE.md",
            AssetKind::Instructions,
            LoadMode::Always,
            src.to_owned(),
        )
    }

    /// A finding of `rule_id` covering 1-based `line` of `a`.
    fn at(a: &Asset, rule_id: &'static str, line: u32) -> Finding {
        Finding {
            rule_id,
            severity: Severity::Medium,
            location: a.span_to_location(a.lines.line_span(&a.source, line)),
            message: String::new(),
            remediation: None,
            token_waste: 0,
            fingerprint: 0,
        }
    }

    fn run(a: &Asset, findings: Vec<Finding>, toml: &str) -> (Vec<String>, usize) {
        let config = Config::parse(toml, Path::new("pulse.toml")).unwrap();
        let ignores = Ignores::new(&config).unwrap();
        let (mut out, n) = apply(findings, &[a], Path::new("/r"), &ignores, |_| true);
        crate::post::sort(&mut out);
        let out = out
            .iter()
            .map(|f| format!("{}:{} {}", f.location.line, f.rule_id, f.message))
            .collect();
        (out, n)
    }

    #[test]
    fn next_line_form_skips_blanks_and_stacked_comments() {
        let a = asset(
            "# T\n<!-- pulse-ignore AI-COG-001 -- ok -->\n\n<!-- pulse-ignore AI-EFF-001 -->\nbad\nbad\n",
        );
        let f = vec![
            at(&a, "AI-COG-001", 5),
            at(&a, "AI-EFF-001", 5),
            at(&a, "AI-COG-001", 6),
        ];
        let (out, n) = run(&a, f, "");
        assert_eq!(out, ["6:AI-COG-001 "]);
        assert_eq!(n, 2);
    }

    #[test]
    fn trailing_form_suppresses_same_line_only() {
        let a = asset("# T\nbad <!-- pulse-ignore AI-COG-001, AI-EFF-001 -->\nbad\n");
        let f = vec![at(&a, "AI-COG-001", 2), at(&a, "AI-COG-001", 3)];
        let (out, n) = run(&a, f, "");
        assert_eq!(
            out,
            [
                "2:AI-STR-002 unused suppression: AI-EFF-001",
                "3:AI-COG-001 "
            ]
        );
        assert_eq!(n, 1);
    }

    #[test]
    fn file_form_requires_position_before_first_heading() {
        let a = asset("<!-- pulse-ignore-file AI-COG-001 -->\n# T\nbad\nbad\n");
        let f = vec![at(&a, "AI-COG-001", 3), at(&a, "AI-COG-001", 4)];
        assert_eq!(run(&a, f, ""), (vec![], 2));

        let late = asset("# T\n<!-- pulse-ignore-file AI-COG-001 -->\nbad\n");
        let (out, n) = run(&late, vec![at(&late, "AI-COG-001", 3)], "");
        assert_eq!(
            out,
            [
                "2:AI-STR-002 invalid suppression: pulse-ignore-file must appear before the first heading",
                "3:AI-COG-001 ",
            ]
        );
        assert_eq!(n, 0);
    }

    #[test]
    fn critical_security_rules_ignore_inline_suppressions() {
        // ADR-006: a planted comment must not hide AI-SEC-002.
        let a = asset(
            "<!-- pulse-ignore-file AI-SEC-002 -->\n# T\nbad <!-- pulse-ignore AI-SEC-002 -->\n",
        );
        let (out, n) = run(&a, vec![at(&a, "AI-SEC-002", 3)], "");
        assert_eq!(
            out,
            [
                "1:AI-STR-002 suppression ignored: AI-SEC-002 can only be suppressed in pulse.toml",
                "3:AI-SEC-002 ",
                "3:AI-STR-002 suppression ignored: AI-SEC-002 can only be suppressed in pulse.toml",
            ]
        );
        assert_eq!(n, 0);
    }

    #[test]
    fn unused_suppression_is_reported_only_for_rules_that_ran() {
        let a = asset("# T\n<!-- pulse-ignore AI-COG-001 -->\nfine\n");
        let (out, _) = run(&a, vec![], "");
        assert_eq!(out, ["2:AI-STR-002 unused suppression: AI-COG-001"]);

        let ignores = Ignores::new(&Config::default()).unwrap();
        let (out, _) = apply(vec![], &[&a], Path::new("/r"), &ignores, |id| {
            id != "AI-COG-001"
        });
        assert!(out.is_empty());
    }

    #[test]
    fn unknown_rule_ids_are_reported() {
        let a = asset("# T\n<!-- pulse-ignore AI-COG-01, X-ACME-001 -->\nx\n");
        let (out, _) = apply(
            vec![],
            &[&a],
            Path::new("/r"),
            &Ignores::new(&Config::default()).unwrap(),
            |id| pulse_rules::find(id).is_some(),
        );
        let msgs: Vec<_> = out.iter().map(|f| f.message.as_str()).collect();
        assert_eq!(msgs, ["invalid suppression: unknown rule AI-COG-01"]);
    }

    #[test]
    fn many_stacked_comments_stay_linear() {
        let mut src = String::from("# T\n");
        for _ in 0..20_000 {
            src += "<!-- pulse-ignore AI-COG-001 -->\n\n";
        }
        src += "bad\n";
        let a = asset(&src);
        let line = a.lines.line_count() as u32 - 1;
        let (out, n) = run(&a, vec![at(&a, "AI-COG-001", line)], "");
        assert_eq!((out.len(), n), (0, 1), "stacked comments share one target");
    }

    #[test]
    fn malformed_comments_are_reported_or_ignored() {
        let a = asset(
            "# T\n<!-- pulse-ignore all -->\n<!-- pulse-ignore -->\n<!-- pulse-ignored X -->\n```\n<!-- pulse-ignore AI-COG-001 -->\n```\nx\n",
        );
        let (out, _) = run(&a, vec![], "");
        assert_eq!(
            out,
            [
                "2:AI-STR-002 invalid suppression: `all` is not accepted; list the rule IDs",
                "3:AI-STR-002 invalid suppression: no rule IDs listed",
            ]
        );
    }

    #[test]
    fn config_ignore_matches_path_glob_and_rules() {
        let a = asset("# T\nbad\n");
        let f = vec![at(&a, "AI-SEC-002", 2), at(&a, "AI-COG-001", 2)];
        let toml =
            "[[ignore]]\npath = \"docs/**\"\nrules = [\"AI-SEC-002\"]\nreason = \"samples\"\n";
        let (out, n) = run(&a, f.clone(), toml);
        assert_eq!((out, n), (vec!["2:AI-COG-001 ".to_owned()], 1));

        let (out, n) = run(&a, f.clone(), "[[ignore]]\npath = \"docs/*.md\"\n");
        assert_eq!((out.len(), n), (0, 2), "omitted rules = every rule");

        let (out, n) = run(&a, f, "[[ignore]]\npath = \"*.md\"\n");
        assert_eq!((out.len(), n), (2, 0), "`*` does not cross `/`");
    }

    #[test]
    fn config_ignore_rejects_unknown_rules_and_bad_globs() {
        let err = |toml: &str| {
            let c = Config::parse(toml, Path::new("pulse.toml")).unwrap();
            Ignores::new(&c).err().unwrap().to_string()
        };
        let msg = err("[[ignore]]\npath = \"a\"\nrules = [\"AI-SEC-02\"]\n");
        assert!(msg.contains("did you mean `AI-SEC-002`"), "{msg}");
        let msg = err("[[ignore]]\npath = \"a[\"\n");
        assert!(msg.contains("invalid glob"), "{msg}");
    }
}
