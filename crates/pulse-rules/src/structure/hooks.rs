//! AI-STR-006: invalid hook definition (Claude settings / plugin hooks, Cursor hooks).

use std::path::Path;

use pulse_core::parse::{HookCmd, HookEntry};
use pulse_core::{Asset, Category, Finding, KindSet, Rule, RuleCtx, RuleMeta, Severity, Span};

use crate::lexicon::platform::{
    CLAUDE_HOOK_EVENTS, CLAUDE_HOOK_TYPES, CURSOR_HOOK_EVENTS, CURSOR_HOOK_TYPES,
};
use crate::security::shell::scan;
use crate::util::{suggest, truncate};

pub static META: RuleMeta = RuleMeta {
    id: "AI-STR-006",
    name: "Invalid hook definition",
    category: Category::Structure,
    default_severity: Severity::High,
    kinds: KindSet::SETTINGS.union(KindSet::HOOKS),
    summary: "A hook has an unknown event or type, no command, a broken matcher or script path.",
    remediation: "Fix the hook so the agent runs it: valid event, type, matcher, timeout and path.",
    inline_suppressible: true,
};

/// Chars of a value quoted in a message.
const QUOTED: usize = 40;
/// Longest timeout, in seconds, before it is flagged.
const MAX_TIMEOUT: f64 = 600.0;
/// Compiled size cap for user matchers; larger ones are not reported.
const MATCHER_SIZE: usize = 1 << 20;
/// Extensions of script files referenced as hook arguments.
const SCRIPT_EXTS: &[&str] = &[
    "bash", "cjs", "js", "mjs", "php", "pl", "ps1", "py", "rb", "sh", "ts", "zsh",
];

pub struct InvalidHook;

/// The hook dialect of an asset and where its relative paths resolve.
struct Dialect<'a> {
    events: &'static [&'static str],
    types: &'static [&'static str],
    /// Scan-root-relative project root (`""` or `dir/`); `None` for plugin hooks.
    project: Option<&'a str>,
    /// Scan-root-relative plugin root, for `${CLAUDE_PLUGIN_ROOT}`.
    plugin: Option<&'a str>,
}

fn dialect(rel: &str) -> Dialect<'_> {
    let prefix = |dir: &str| {
        let at = if rel.starts_with(dir) {
            Some(0)
        } else {
            rel.find(&format!("/{dir}")).map(|i| i + 1)
        };
        at.map(|i| &rel[..i])
    };
    if let Some(project) = prefix(".cursor/") {
        return Dialect {
            events: CURSOR_HOOK_EVENTS,
            types: CURSOR_HOOK_TYPES,
            project: Some(project),
            plugin: None,
        };
    }
    let claude = |project, plugin| Dialect {
        events: CLAUDE_HOOK_EVENTS,
        types: CLAUDE_HOOK_TYPES,
        project,
        plugin,
    };
    match prefix(".claude/") {
        Some(project) => claude(Some(project), None),
        // Plugin `hooks/hooks.json`: the plugin root is the parent of `hooks/`.
        None if rel.ends_with("hooks/hooks.json") => {
            claude(None, Some(&rel[..rel.len() - "hooks/hooks.json".len()]))
        }
        None => claude(Some(""), None),
    }
}

impl Rule for InvalidHook {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, asset: &Asset, ctx: &RuleCtx<'_>, out: &mut Vec<Finding>) {
        let d = dialect(&asset.rel_path);
        let mut push = |span: Span, severity: Severity, message: String| {
            let mut f = Finding::new(&META, asset, span, message);
            f.severity = severity;
            out.push(f);
        };
        for entry in asset.hooks() {
            if !d.events.contains(&entry.event.as_str()) {
                let hint = suggest(&entry.event, d.events)
                    .map_or_else(String::new, |s| format!(" (did you mean `{s}`?)"));
                let event = truncate(&entry.event, QUOTED);
                push(
                    entry.event_span,
                    Severity::High,
                    format!("Unknown hook event `{event}`{hint}"),
                );
            }
            if let Some(message) = matcher_error(entry) {
                let span = entry.matcher_span.unwrap_or(entry.event_span);
                push(span, Severity::High, message);
            }
            for h in &entry.hooks {
                check_cmd(h, &d, asset, ctx, &mut push);
            }
        }
    }
}

fn check_cmd(
    h: &HookCmd,
    d: &Dialect<'_>,
    asset: &Asset,
    ctx: &RuleCtx<'_>,
    push: &mut impl FnMut(Span, Severity, String),
) {
    if !d.types.contains(&h.kind.as_str()) {
        let kind = truncate(&h.kind, QUOTED);
        let span = h.kind_span.unwrap_or(h.command_span);
        push(span, Severity::High, format!("Unknown hook type `{kind}`"));
    } else if matches!(h.kind.as_str(), "command" | "prompt") && h.command.trim().is_empty() {
        let kind = &h.kind;
        push(
            h.command_span,
            Severity::High,
            format!("`{kind}` hook has no `{kind}`"),
        );
    }
    if let Some(t) = h.timeout
        && (t <= 0.0 || t > MAX_TIMEOUT)
    {
        push(
            h.timeout_span.unwrap_or(h.command_span),
            Severity::Medium,
            format!("Hook timeout {t} s must be > 0 and <= 600 s"),
        );
    }
    if h.kind == "command" {
        for (start, end, path) in missing_scripts(&h.command, d, ctx) {
            let span = crate::security::mcp::value_span(
                asset,
                h.command_span,
                &h.command,
                start,
                end - start,
            );
            let path = truncate(&path, QUOTED);
            push(
                span,
                Severity::Medium,
                format!("Hook script `{path}` does not exist"),
            );
        }
    }
}

/// Why the matcher of `entry` cannot work, if it cannot.
fn matcher_error(entry: &HookEntry) -> Option<String> {
    let m = entry.matcher.as_deref()?;
    // `""`, `*` and plain names or `|`/`,` lists are not regexes (Claude docs).
    let plain = m
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b" _-,|".contains(&b));
    if m == "*" || plain {
        return None;
    }
    // AI-STR-006 FP guard: matchers are JavaScript regexes; look-around and backreferences
    // are valid there but unsupported by the `regex` crate.
    if ["(?=", "(?!", "(?<=", "(?<!", "\\k<"]
        .iter()
        .any(|js| m.contains(js))
        || m.as_bytes()
            .windows(2)
            .any(|w| w[0] == b'\\' && (b'1'..=b'9').contains(&w[1]))
    {
        return None;
    }
    // User content: compiled once per hook entry, with a size cap.
    match regex::RegexBuilder::new(m).size_limit(MATCHER_SIZE).build() {
        Err(regex::Error::Syntax(_)) => Some(format!(
            "Hook matcher `{}` is not a valid regex",
            truncate(m, QUOTED)
        )),
        _ => None,
    }
}

/// Script paths of `command` that resolve inside the project but do not exist, as
/// `(start, end, path)` in `command`.
fn missing_scripts(
    command: &str,
    d: &Dialect<'_>,
    ctx: &RuleCtx<'_>,
) -> Vec<(usize, usize, String)> {
    let mut out = Vec::new();
    for p in scan(command, 0, command.len()) {
        for stage in &p.stages {
            for (i, w) in stage.iter().enumerate() {
                // Redirect targets are outputs, not scripts.
                let redirected = w.raw.starts_with(['>', '<'])
                    || i.checked_sub(1)
                        .and_then(|j| stage.get(j))
                        .is_some_and(|prev| prev.raw.ends_with(['>', '<']));
                if redirected || w.raw.starts_with('-') {
                    continue;
                }
                let lit = w.literal();
                let Some(rel) = resolve(&lit, d) else {
                    continue;
                };
                if !exists(&rel, ctx) {
                    out.push((w.start, w.end, lit.into_owned()));
                }
            }
        }
    }
    out
}

/// The scan-root-relative path a hook argument names, when it is a checkable script path.
fn resolve(word: &str, d: &Dialect<'_>) -> Option<String> {
    let strip = |vars: &[&str]| {
        vars.iter()
            .find_map(|v| word.strip_prefix(v).and_then(|r| r.strip_prefix('/')))
    };
    let (base, rest, anchored) =
        if let Some(rest) = strip(&["$CLAUDE_PROJECT_DIR", "${CLAUDE_PROJECT_DIR}"]) {
            (d.project?, rest, true)
        } else if let Some(rest) = strip(&["$CLAUDE_PLUGIN_ROOT", "${CLAUDE_PLUGIN_ROOT}"]) {
            (d.plugin?, rest, true)
        } else if let Some(rest) = word.strip_prefix("./") {
            (d.project?, rest, false)
        } else if !word.starts_with(['/', '~', '$']) && word.contains('/') {
            (d.project?, word, false)
        } else {
            return None;
        };
    // Unexpanded variables, globs and dependencies (`node_modules`) are not checked.
    if rest.is_empty()
        || rest.contains(['$', '*', '?', '[', '`', '='])
        || rest.split('/').any(|s| s == ".." || s == "node_modules")
    {
        return None;
    }
    // Bare relative words need a script extension (`git -C src/app` is not a script).
    let ext = Path::new(rest).extension().and_then(|e| e.to_str());
    if !anchored && !ext.is_some_and(|e| SCRIPT_EXTS.contains(&e)) {
        return None;
    }
    Some(format!("{base}{rest}"))
}

/// Whether the walker saw `rel`, falling back to the disk for ignored files.
fn exists(rel: &str, ctx: &RuleCtx<'_>) -> bool {
    ctx.file_exists(rel) || ctx.root.join(rel).exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialect_by_path() {
        let d = dialect("app/.cursor/hooks.json");
        assert_eq!(d.events, CURSOR_HOOK_EVENTS);
        assert_eq!(d.project, Some("app/"));
        let d = dialect(".claude/settings.json");
        assert_eq!((d.project, d.plugin), (Some(""), None));
        let d = dialect("plugins/fmt/hooks/hooks.json");
        assert_eq!((d.project, d.plugin), (None, Some("plugins/fmt/")));
    }

    #[test]
    fn resolve_only_checkable_paths() {
        let d = dialect("app/.claude/settings.json");
        let r = |w| resolve(w, &d);
        assert_eq!(
            r("$CLAUDE_PROJECT_DIR/.claude/hooks/fmt").as_deref(),
            Some("app/.claude/hooks/fmt")
        );
        assert_eq!(
            r("./scripts/lint.sh").as_deref(),
            Some("app/scripts/lint.sh")
        );
        assert_eq!(r("scripts/lint.py").as_deref(), Some("app/scripts/lint.py"));
        for skip in [
            "src/app",
            "/usr/bin/env",
            "~/bin/x.sh",
            "$HOME/x.sh",
            "./node_modules/.bin/x.js",
            "${CLAUDE_PLUGIN_ROOT}/x.sh",
            "./*.sh",
        ] {
            assert_eq!(r(skip), None, "{skip}");
        }
    }

    #[test]
    fn matcher_validity() {
        let entry = |m: &str| HookEntry {
            event: "PreToolUse".into(),
            event_span: Span::default(),
            matcher: Some(m.into()),
            matcher_span: None,
            hooks: Vec::new(),
        };
        for ok in ["", "*", "Edit|Write", "mcp__.*", "^Notebook", "(?!Bash).*"] {
            assert_eq!(matcher_error(&entry(ok)), None, "{ok}");
        }
        for bad in ["Edit(", "[Bash", "*Bash"] {
            assert!(matcher_error(&entry(bad)).is_some(), "{bad}");
        }
    }
}
