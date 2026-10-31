//! AI-STR-006: invalid hook definition (Claude settings / plugin hooks, Cursor, Copilot and
//! Kiro hooks).

use std::path::Path;

use pulse_core::parse::{HookCmd, HookEntry, HookFormat};
use pulse_core::{Asset, Category, Finding, KindSet, Rule, RuleCtx, RuleMeta, Severity, Span};

use crate::lexicon::platform::{
    CLAUDE_HOOK_EVENTS, CLAUDE_HOOK_TYPES, COPILOT_HOOK_EVENTS, COPILOT_HOOK_TYPES,
    CURSOR_HOOK_EVENTS, CURSOR_HOOK_TYPES, KIRO_HOOK_TRIGGERS, KIRO_HOOK_TYPES,
    KIRO_LEGACY_HOOK_EVENTS, KIRO_LEGACY_HOOK_TYPES,
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

/// Hook types that need a text field, as `(type, field)`.
const STANDARD_FIELDS: &[(&str, &str)] = &[("command", "command"), ("prompt", "prompt")];
const KIRO_FIELDS: &[(&str, &str)] = &[("agent", "prompt"), ("command", "command")];
const KIRO_LEGACY_FIELDS: &[(&str, &str)] = &[("askAgent", "prompt"), ("runCommand", "command")];

/// The hook dialect of an asset and where its relative paths resolve.
struct Dialect<'a> {
    events: &'static [&'static str],
    types: &'static [&'static str],
    /// `(type, field)` pairs: a hook of that type needs that text.
    fields: &'static [(&'static str, &'static str)],
    /// Kiro: `(event key, action key)`, `0` timeouts allowed, wildcard and `@` matchers.
    kiro: Option<(&'static str, &'static str)>,
    /// Scan-root-relative project root (`""` or `dir/`); `None` for plugin hooks.
    project: Option<&'a str>,
    /// Scan-root-relative plugin root, for `${CLAUDE_PLUGIN_ROOT}`.
    plugin: Option<&'a str>,
}

/// The dialect of the hooks at `rel` in `format`.
fn dialect(rel: &str, format: HookFormat) -> Dialect<'_> {
    let prefix = |dir: &str| {
        let at = if rel.starts_with(dir) {
            Some(0)
        } else {
            rel.find(&format!("/{dir}")).map(|i| i + 1)
        };
        at.map(|i| &rel[..i])
    };
    // Kiro runs commands in the project root; a library copy elsewhere is not checked for
    // script paths.
    let kiro = |events, types, fields, keys| Dialect {
        events,
        types,
        fields,
        kiro: Some(keys),
        project: prefix(".kiro/hooks/"),
        plugin: None,
    };
    match format {
        HookFormat::KiroLegacy => {
            return kiro(
                KIRO_LEGACY_HOOK_EVENTS,
                KIRO_LEGACY_HOOK_TYPES,
                KIRO_LEGACY_FIELDS,
                ("when.type", "then"),
            );
        }
        HookFormat::Kiro => {
            return kiro(
                KIRO_HOOK_TRIGGERS,
                KIRO_HOOK_TYPES,
                KIRO_FIELDS,
                ("trigger", "action"),
            );
        }
        HookFormat::Standard | HookFormat::Copilot => {}
    }
    if let Some(project) = prefix(".cursor/") {
        return Dialect {
            events: CURSOR_HOOK_EVENTS,
            types: CURSOR_HOOK_TYPES,
            fields: STANDARD_FIELDS,
            kiro: None,
            project: Some(project),
            plugin: None,
        };
    }
    // Copilot: scripts resolve from the repository root once installed in `.github/hooks/`;
    // a library copy elsewhere (`hooks/<name>/hooks.json`) is not checked for them.
    let installed = prefix(".github/hooks/");
    if format == HookFormat::Copilot || installed.is_some() {
        return Dialect {
            events: COPILOT_HOOK_EVENTS,
            types: COPILOT_HOOK_TYPES,
            fields: STANDARD_FIELDS,
            kiro: None,
            project: installed,
            plugin: None,
        };
    }
    let claude = |project, plugin| Dialect {
        events: CLAUDE_HOOK_EVENTS,
        types: CLAUDE_HOOK_TYPES,
        fields: STANDARD_FIELDS,
        kiro: None,
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
        let d = dialect(&asset.rel_path, asset.settings().hook_format);
        for entry in asset.hooks() {
            let mut push = |span: Span, severity: Severity, message: String| {
                let message = message + entry.disabled_note();
                let mut f = Finding::new(&META, asset, span, message);
                f.severity = severity;
                out.push(f);
            };
            if let Some((event_key, action_key)) = d.kiro {
                if entry.event.is_empty() {
                    push(
                        entry.event_span,
                        Severity::High,
                        format!("Hook has no `{event_key}`"),
                    );
                }
                if entry.hooks.is_empty() {
                    push(
                        entry.event_span,
                        Severity::High,
                        format!("Hook has no `{action_key}`"),
                    );
                }
            }
            if !entry.event.is_empty() && !d.events.contains(&entry.event.as_str()) {
                let hint = suggest(&entry.event, d.events)
                    .map_or_else(String::new, |s| format!(" (did you mean `{s}`?)"));
                let event = truncate(&entry.event, QUOTED);
                let what = match d.kiro {
                    Some((key, _)) if key.contains('.') => format!("`{key}`"),
                    Some((key, _)) => key.to_owned(),
                    None => "event".to_owned(),
                };
                push(
                    entry.event_span,
                    Severity::High,
                    format!("Unknown hook {what} `{event}`{hint}"),
                );
            }
            if let Some(message) = matcher_error(entry, d.kiro.is_some()) {
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
    let field = d.fields.iter().find(|(t, _)| *t == h.kind).map(|f| f.1);
    if h.kind.is_empty() {
        let action = d.kiro.map_or("hook", |k| k.1);
        push(
            h.command_span,
            Severity::High,
            format!("Hook `{action}` has no `type`"),
        );
    } else if !d.types.contains(&h.kind.as_str()) {
        let kind = truncate(&h.kind, QUOTED);
        let hint = if d.kiro.is_some() {
            suggest(&h.kind, d.types)
                .map_or_else(String::new, |s| format!(" (did you mean `{s}`?)"))
        } else {
            String::new()
        };
        let span = h.kind_span.unwrap_or(h.command_span);
        push(
            span,
            Severity::High,
            format!("Unknown hook type `{kind}`{hint}"),
        );
    } else if let Some(field) = field
        && h.command.trim().is_empty()
    {
        let kind = &h.kind;
        push(
            h.command_span,
            Severity::High,
            format!("`{kind}` hook has no `{field}`"),
        );
    }
    // Kiro: `0` disables the timeout.
    let min_ok = |t: f64| if d.kiro.is_some() { t >= 0.0 } else { t > 0.0 };
    if let Some(t) = h.timeout
        && (!min_ok(t) || t > MAX_TIMEOUT)
    {
        let bound = if d.kiro.is_some() { ">= 0" } else { "> 0" };
        push(
            h.timeout_span.unwrap_or(h.command_span),
            Severity::Medium,
            format!("Hook timeout {t} s must be {bound} and <= 600 s"),
        );
    }
    if field == Some("command") {
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

/// Why the matcher of `entry` cannot work, if it cannot; `kiro` also accepts tool selectors
/// (`@mcp`, `@server/tool`) and `*`/`?` wildcards.
fn matcher_error(entry: &HookEntry, kiro: bool) -> Option<String> {
    let m = entry.matcher.as_deref()?;
    // AI-STR-006 FP guard: Kiro selectors and wildcard patterns are not regexes.
    let wildcard = || !m.contains(['(', ')', '[', ']', '{', '}', '\\', '+', '|', '^', '$']);
    if kiro && (m.starts_with('@') || wildcard()) {
        return None;
    }
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
        let d = dialect("app/.cursor/hooks.json", HookFormat::Standard);
        assert_eq!(d.events, CURSOR_HOOK_EVENTS);
        assert_eq!(d.project, Some("app/"));
        let d = dialect(".claude/settings.json", HookFormat::Standard);
        assert_eq!((d.project, d.plugin), (Some(""), None));
        let d = dialect("plugins/fmt/hooks/hooks.json", HookFormat::Standard);
        assert_eq!((d.project, d.plugin), (None, Some("plugins/fmt/")));
        let d = dialect(".github/hooks/audit.json", HookFormat::Standard);
        assert_eq!((d.events, d.project), (COPILOT_HOOK_EVENTS, Some("")));
        let d = dialect("hooks/audit/hooks.json", HookFormat::Copilot);
        assert_eq!((d.events, d.project), (COPILOT_HOOK_EVENTS, None));
        let d = dialect("app/.kiro/hooks/fmt.kiro.hook", HookFormat::KiroLegacy);
        assert_eq!(
            (d.events, d.project),
            (KIRO_LEGACY_HOOK_EVENTS, Some("app/"))
        );
        let d = dialect("hooks/fmt.kiro.hook", HookFormat::KiroLegacy);
        assert_eq!(d.project, None);
        let d = dialect(".kiro/hooks/fmt.json", HookFormat::Kiro);
        assert_eq!((d.types, d.project), (KIRO_HOOK_TYPES, Some("")));
    }

    #[test]
    fn resolve_only_checkable_paths() {
        let d = dialect("app/.claude/settings.json", HookFormat::Standard);
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
            enabled: true,
        };
        for ok in ["", "*", "Edit|Write", "mcp__.*", "^Notebook", "(?!Bash).*"] {
            assert_eq!(matcher_error(&entry(ok), false), None, "{ok}");
        }
        for bad in ["Edit(", "[Bash", "*Bash"] {
            assert!(matcher_error(&entry(bad), false).is_some(), "{bad}");
        }
        // Kiro: selectors and wildcards are fine, broken regexes are not.
        for ok in ["*.ts", "mcp_?", "@postgres/query", "read", r"\.(ts|tsx)$"] {
            assert_eq!(matcher_error(&entry(ok), true), None, "{ok}");
        }
        for bad in [r"\.(ts|tsx$", "*(Bash"] {
            assert!(matcher_error(&entry(bad), true).is_some(), "{bad}");
        }
    }
}
