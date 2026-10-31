//! Settings / hooks view: Claude `settings.json` (`permissions`, `hooks`, `env`, flags) and
//! standalone hook files (Claude plugin `hooks/hooks.json`, Cursor `.cursor/hooks.json`,
//! Copilot `.github/hooks/*.json`, Kiro `.kiro/hooks/*.kiro.hook` and `.kiro/hooks/*.json`), and
//! the inline `hooks` of a Kiro JSON agent.

use crate::model::Span;
use crate::parse::json::{JsonDoc, JsonNode, Shape, ViewError};

/// Parsed settings; a hooks-only file fills just [`SettingsView::hooks`].
#[derive(Debug, Clone, Default)]
pub struct SettingsView {
    pub allow: Vec<(String, Span)>,
    pub deny: Vec<(String, Span)>,
    pub ask: Vec<(String, Span)>,
    /// `permissions.defaultMode`.
    pub default_mode: Option<(String, Span)>,
    /// `enableAllProjectMcpServers`.
    pub enable_all_project_mcp: Option<(bool, Span)>,
    pub hooks: Vec<HookEntry>,
    /// The hook file format, from the document's shape.
    pub hook_format: HookFormat,
    /// `(key, value, value span)`.
    pub env: Vec<(String, String, Span)>,
    pub errors: Vec<ViewError>,
}

/// The format of a file's hooks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum HookFormat {
    /// Claude / Cursor: a `hooks` object keyed by event.
    #[default]
    Standard,
    /// Copilot: the standard shape with the Copilot keys (`bash`, `powershell`, `timeoutSec`).
    Copilot,
    /// Kiro IDE 0.x `*.kiro.hook`: one hook with `when { type }` and `then { type }`.
    KiroLegacy,
    /// Kiro v1 (`.kiro/hooks/*.json`, agent `hooks` arrays): `hooks: [{ trigger, matcher,
    /// action { type } }]`.
    Kiro,
    /// Kiro CLI 2.x agent `hooks`: an object keyed by camelCase event, `[{ matcher, command }]`.
    KiroAgent,
}

/// One matcher group under a hook event (a Kiro hook is one entry).
#[derive(Debug, Clone, PartialEq)]
pub struct HookEntry {
    /// The event (Kiro: `trigger`, legacy `when.type`).
    pub event: String,
    /// The event value, or the event key; for a Kiro hook without one, the hook object.
    pub event_span: Span,
    /// `None` when absent (matches everything).
    pub matcher: Option<String>,
    pub matcher_span: Option<Span>,
    pub hooks: Vec<HookCmd>,
    /// Kiro `enabled` (`true` when absent and for other formats).
    pub enabled: bool,
}

impl HookEntry {
    /// Appended to finding messages about this entry: says when it is disabled.
    pub fn disabled_note(&self) -> &'static str {
        if self.enabled {
            ""
        } else {
            " (the hook is disabled)"
        }
    }
}

/// One hook action.
#[derive(Debug, Clone, PartialEq)]
pub struct HookCmd {
    /// `type` (`command`, `prompt`, Kiro `agent`, legacy `runCommand`/`askAgent`, …);
    /// `command` when absent.
    pub kind: String,
    /// The `type` value, when present.
    pub kind_span: Option<Span>,
    /// The `command` (Copilot: `bash`, else `powershell`), or the `prompt` text of a prompt
    /// or agent hook; empty for other types and when missing (AI-STR-006 reports a missing
    /// one).
    pub command: String,
    /// The command text, or the whole hook object when there is none.
    pub command_span: Span,
    /// Whether `command` is shell text, not a prompt.
    pub shell: bool,
    /// Seconds (`timeout`, Copilot `timeoutSec`; Kiro `0` disables it).
    pub timeout: Option<f64>,
    /// The timeout value, when it is a number.
    pub timeout_span: Option<Span>,
}

impl SettingsView {
    pub fn build(doc: &JsonDoc) -> Self {
        let mut v = Self::default();
        let mut sh = Shape::default();
        let Some(root) = &doc.root else {
            return v;
        };
        // Kiro: a legacy hook has `when`/`then` at the root, a v1 file a `hooks` array.
        if root.get("when").is_some() || root.get("then").is_some() {
            v.hook_format = HookFormat::KiroLegacy;
            v.hooks.extend(kiro_legacy(&mut sh, root));
            v.errors = sh.errors;
            return v;
        }
        // A Claude `"hooks": []` is not Kiro: v1 files carry `version: "v1"`.
        let v1 = root.get("version").and_then(JsonNode::as_str) == Some("v1");
        if let Some(list) = root.get("hooks").filter(|n| v1 && n.as_array().is_some()) {
            v.hook_format = HookFormat::Kiro;
            for h in sh.array(list, "hooks") {
                v.hooks.extend(kiro_hook(&mut sh, h));
            }
            v.errors = sh.errors;
            return v;
        }
        for m in sh.object(root, "(root)") {
            let n = &m.value;
            match m.key.as_str() {
                "permissions" => {
                    for p in sh.object(n, "permissions") {
                        match p.key.as_str() {
                            "allow" => v.allow = sh.strings(&p.value, "allow"),
                            "deny" => v.deny = sh.strings(&p.value, "deny"),
                            "ask" => v.ask = sh.strings(&p.value, "ask"),
                            "defaultMode" => v.default_mode = sh.string(&p.value, "defaultMode"),
                            _ => {}
                        }
                    }
                }
                "enableAllProjectMcpServers" => {
                    v.enable_all_project_mcp = sh
                        .bool(n, "enableAllProjectMcpServers")
                        .map(|b| (b, n.span));
                }
                "env" => v.env = sh.string_map(n, "env"),
                "hooks" if hooks(&mut sh, n, &mut v.hooks) => v.hook_format = HookFormat::Copilot,
                _ => {}
            }
        }
        v.errors = sh.errors;
        v
    }

    /// The inline `hooks` of a Kiro JSON agent: the CLI 2.x object keyed by event or the v1
    /// array (ADR-053).
    pub fn build_agent(doc: &JsonDoc) -> Self {
        let mut v = Self::default();
        let mut sh = Shape::default();
        let Some(list) = doc.root.as_ref().and_then(|r| r.get("hooks")) else {
            return v;
        };
        if list.as_array().is_some() {
            v.hook_format = HookFormat::Kiro;
            for h in sh.array(list, "hooks") {
                v.hooks.extend(kiro_hook(&mut sh, h));
            }
        } else {
            v.hook_format = HookFormat::KiroAgent;
            hooks(&mut sh, list, &mut v.hooks);
        }
        v.errors = sh.errors;
        v
    }
}

/// Reads the `hooks` object into `out`; whether a hook uses the Copilot keys.
fn hooks(sh: &mut Shape, node: &JsonNode, out: &mut Vec<HookEntry>) -> bool {
    let mut copilot = false;
    for ev in sh.object(node, "hooks") {
        for group in sh.array(&ev.value, &ev.key) {
            if group.as_object().is_none() {
                sh.object(group, &ev.key);
                continue;
            }
            let matcher = group.get("matcher").and_then(|n| sh.string(n, "matcher"));
            let mut cmds = Vec::new();
            match group.get("hooks") {
                // Claude: `{ matcher, hooks: [{ type, command, timeout }] }`.
                Some(list) => {
                    for h in sh.array(list, "hooks") {
                        if h.as_object().is_none() {
                            sh.object(h, "hooks[]");
                        } else if let Some(c) = cmd(sh, h, &mut copilot) {
                            cmds.push(c);
                        }
                    }
                }
                // Cursor `{ command }`, Copilot `{ type, bash, powershell, timeoutSec }`
                // directly under the event.
                None => cmds.extend(cmd(sh, group, &mut copilot)),
            }
            out.push(HookEntry {
                event: ev.key.clone(),
                event_span: ev.key_span,
                matcher_span: matcher.as_ref().map(|m| m.1),
                matcher: matcher.map(|m| m.0),
                hooks: cmds,
                enabled: true,
            });
        }
    }
    copilot
}

fn cmd(sh: &mut Shape, node: &JsonNode, copilot: &mut bool) -> Option<HookCmd> {
    let kind = node.get("type").and_then(|n| sh.string(n, "type"));
    let kind_span = kind.as_ref().map(|k| k.1);
    let kind = kind.map_or_else(|| "command".to_owned(), |t| t.0);
    let shell = node.get("bash").or_else(|| node.get("powershell"));
    let timeout_sec = node.get("timeoutSec");
    *copilot |= shell.is_some() || timeout_sec.is_some();
    let text = node.get("command").or(shell).or_else(|| node.get("prompt"));
    let (command, command_span) = match text {
        Some(t) => sh.string(t, "command")?,
        // No command text: other hook types (`http`, `agent`, …) carry none; a missing one
        // is AI-STR-006's to report.
        None => (String::new(), node.span),
    };
    let timeout = node
        .get("timeout")
        .or(timeout_sec)
        .and_then(|n| sh.number(n, "timeout").map(|t| (t, n.span)));
    Some(HookCmd {
        shell: kind == "command",
        kind,
        kind_span,
        command,
        command_span,
        timeout: timeout.map(|t| t.0),
        timeout_span: timeout.map(|t| t.1),
    })
}

/// A legacy Kiro hook: `{ enabled, when { type, patterns }, then { type, command | prompt,
/// timeout } }`. Legacy `patterns` and `toolTypes` are globs and categories, not matchers.
fn kiro_legacy(sh: &mut Shape, root: &JsonNode) -> Option<HookEntry> {
    let when = root.get("when");
    let event = when
        .and_then(|w| sh.object(w, "when").iter().rev().find(|m| m.key == "type"))
        .and_then(|m| sh.string(&m.value, "when.type"));
    let cmd = root
        .get("then")
        .and_then(|t| kiro_action(sh, t, "runCommand", "then"));
    Some(HookEntry {
        event_span: event.as_ref().map_or(when.unwrap_or(root).span, |e| e.1),
        event: event.map(|e| e.0).unwrap_or_default(),
        matcher: None,
        matcher_span: None,
        hooks: cmd.into_iter().collect(),
        enabled: enabled(sh, root),
    })
}

/// A Kiro v1 hook: `{ name, trigger, matcher, action { type, command | prompt }, timeout,
/// enabled }`.
fn kiro_hook(sh: &mut Shape, node: &JsonNode) -> Option<HookEntry> {
    if node.as_object().is_none() {
        sh.object(node, "hooks[]");
        return None;
    }
    let trigger = node.get("trigger").and_then(|n| sh.string(n, "trigger"));
    let matcher = node.get("matcher").and_then(|n| sh.string(n, "matcher"));
    let mut cmd = node
        .get("action")
        .and_then(|a| kiro_action(sh, a, "command", "action"));
    if let Some(c) = &mut cmd
        && let Some(t) = node.get("timeout")
        && let Some(secs) = sh.number(t, "timeout")
    {
        (c.timeout, c.timeout_span) = (Some(secs), Some(t.span));
    }
    Some(HookEntry {
        event_span: trigger.as_ref().map_or(node.span, |t| t.1),
        event: trigger.map(|t| t.0).unwrap_or_default(),
        matcher_span: matcher.as_ref().map(|m| m.1),
        matcher: matcher.map(|m| m.0),
        hooks: cmd.into_iter().collect(),
        enabled: enabled(sh, node),
    })
}

/// A Kiro action object (`what`): the `shell` type reads `command`, the others `prompt`; a
/// missing `type` stays empty and reads `command` when present, so AI-SEC-008/009 see it.
fn kiro_action(sh: &mut Shape, node: &JsonNode, shell: &str, what: &str) -> Option<HookCmd> {
    if node.as_object().is_none() {
        sh.object(node, what);
        return None;
    }
    let kind = node.get("type").and_then(|n| sh.string(n, "type"));
    let kind_span = kind.as_ref().map(|k| k.1);
    let kind = kind.map(|k| k.0).unwrap_or_default();
    let key = if kind == shell || (kind.is_empty() && node.get("command").is_some()) {
        "command"
    } else {
        "prompt"
    };
    // A non-string text is a shape error, and the action stays (with no text).
    let (command, command_span) = match node.get(key) {
        Some(t) => sh.string(t, key).unwrap_or((String::new(), t.span)),
        None => (String::new(), node.span),
    };
    let timeout = node
        .get("timeout")
        .and_then(|n| sh.number(n, "timeout").map(|t| (t, n.span)));
    Some(HookCmd {
        shell: key == "command",
        kind,
        kind_span,
        command,
        command_span,
        timeout: timeout.map(|t| t.0),
        timeout_span: timeout.map(|t| t.1),
    })
}

/// Kiro `enabled`, `true` when absent.
fn enabled(sh: &mut Shape, node: &JsonNode) -> bool {
    node.get("enabled")
        .and_then(|n| sh.bool(n, "enabled"))
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copilot_hooks_use_bash_and_timeout_sec() {
        let doc = JsonDoc::parse(
            r#"{"version": 1, "hooks": {"sessionStart": [
                {"type": "command", "bash": "./a.sh", "powershell": "./a.ps1", "timeoutSec": 5}
            ]}}"#,
        );
        let v = SettingsView::build(&doc);
        assert_eq!(v.hook_format, HookFormat::Copilot);
        let h = &v.hooks[0].hooks[0];
        assert_eq!((h.command.as_str(), h.timeout), ("./a.sh", Some(5.0)));
        let doc = JsonDoc::parse(r#"{"hooks": {"stop": [{"command": "x", "timeout": 5}]}}"#);
        assert_eq!(SettingsView::build(&doc).hook_format, HookFormat::Standard);
    }

    #[test]
    fn kiro_legacy_hook() {
        let doc = JsonDoc::parse(
            r#"{"name": "lint", "enabled": false,
                "when": {"type": "fileEdited", "patterns": ["*.ts"]},
                "then": {"type": "runCommand", "command": "npm run lint", "timeout": 30}}"#,
        );
        let v = SettingsView::build(&doc);
        assert_eq!(v.hook_format, HookFormat::KiroLegacy);
        let e = &v.hooks[0];
        assert_eq!(
            (e.event.as_str(), e.matcher.as_deref(), e.enabled),
            ("fileEdited", None, false)
        );
        let h = &e.hooks[0];
        assert_eq!(
            (h.kind.as_str(), h.command.as_str(), h.timeout),
            ("runCommand", "npm run lint", Some(30.0))
        );
        // `askAgent` reads `prompt`, never `command`.
        let doc = JsonDoc::parse(
            r#"{"when": {"type": "agentStop"}, "then": {"type": "askAgent", "command": "x"}}"#,
        );
        let v = SettingsView::build(&doc);
        assert_eq!(v.hooks[0].hooks[0].command, "");
        assert!(v.hooks[0].enabled && v.errors.is_empty());
    }

    #[test]
    fn kiro_v1_hooks() {
        let doc = JsonDoc::parse(
            r#"{"version": "v1", "hooks": [
                {"name": "a", "trigger": "PostFileSave", "matcher": "\\.ts$",
                 "action": {"type": "command", "command": "npx eslint --fix"}, "timeout": 0},
                {"name": "b", "trigger": "Stop", "enabled": false,
                 "action": {"type": "agent", "prompt": "Check coverage"}},
                {"name": "c", "trigger": "Stop", "action": "oops"}
            ]}"#,
        );
        let v = SettingsView::build(&doc);
        assert_eq!(v.hook_format, HookFormat::Kiro);
        assert_eq!(v.hooks.len(), 3);
        let (a, b) = (&v.hooks[0], &v.hooks[1]);
        assert_eq!(
            (a.event.as_str(), a.matcher.as_deref()),
            ("PostFileSave", Some(r"\.ts$"))
        );
        assert_eq!(
            (a.hooks[0].command.as_str(), a.hooks[0].timeout),
            ("npx eslint --fix", Some(0.0))
        );
        assert_eq!(
            (
                b.hooks[0].kind.as_str(),
                b.hooks[0].command.as_str(),
                b.enabled
            ),
            ("agent", "Check coverage", false)
        );
        assert_eq!(b.disabled_note(), " (the hook is disabled)");
        assert!(v.hooks[2].hooks.is_empty());
        assert_eq!(v.errors.len(), 1);
        // Claude settings with an empty `hooks` array stay Claude.
        let doc = JsonDoc::parse(r#"{"hooks": [], "env": {"A": "1"}}"#);
        let v = SettingsView::build(&doc);
        assert_eq!((v.hook_format, v.env.len()), (HookFormat::Standard, 1));
    }

    #[test]
    fn kiro_action_without_type_or_with_bad_text_is_kept() {
        let doc = JsonDoc::parse(r#"{"when": {"type": "agentStop"}, "then": {"command": "x"}}"#);
        let h = &SettingsView::build(&doc).hooks[0].hooks[0];
        assert_eq!((h.kind.as_str(), h.command.as_str()), ("", "x"));
        let doc = JsonDoc::parse(
            r#"{"when": {"type": "agentStop"}, "then": {"type": "runCommand", "command": ["x"]}}"#,
        );
        let v = SettingsView::build(&doc);
        assert_eq!((v.hooks[0].hooks.len(), v.errors.len()), (1, 1));
        assert_eq!(v.hooks[0].hooks[0].command, "");
    }
}
