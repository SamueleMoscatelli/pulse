//! Settings / hooks view: Claude `settings.json` (`permissions`, `hooks`, `env`, flags) and
//! standalone hook files (Claude plugin `hooks/hooks.json`, Cursor `.cursor/hooks.json`,
//! Copilot `.github/hooks/*.json`).

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
    /// Whether a hook uses the Copilot keys (`bash`, `powershell`, `timeoutSec`).
    pub copilot_hooks: bool,
    /// `(key, value, value span)`.
    pub env: Vec<(String, String, Span)>,
    pub errors: Vec<ViewError>,
}

/// One matcher group under a hook event.
#[derive(Debug, Clone, PartialEq)]
pub struct HookEntry {
    pub event: String,
    pub event_span: Span,
    /// `None` when absent (matches everything).
    pub matcher: Option<String>,
    pub matcher_span: Option<Span>,
    pub hooks: Vec<HookCmd>,
}

/// One hook action.
#[derive(Debug, Clone, PartialEq)]
pub struct HookCmd {
    /// `type` (`command`, `prompt`, …); `command` when absent.
    pub kind: String,
    /// The `type` value, when present.
    pub kind_span: Option<Span>,
    /// The `command` (Copilot: `bash`, else `powershell`), or the `prompt` text of a prompt
    /// hook; empty for other types and when
    /// missing (AI-STR-006 reports a missing one).
    pub command: String,
    /// The command text, or the whole hook object when there is none.
    pub command_span: Span,
    /// Seconds (`timeout`, Copilot `timeoutSec`).
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
                "hooks" => v.copilot_hooks = hooks(&mut sh, n, &mut v.hooks),
                _ => {}
            }
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
        kind,
        kind_span,
        command,
        command_span,
        timeout: timeout.map(|t| t.0),
        timeout_span: timeout.map(|t| t.1),
    })
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
        assert!(v.copilot_hooks);
        let h = &v.hooks[0].hooks[0];
        assert_eq!((h.command.as_str(), h.timeout), ("./a.sh", Some(5.0)));
        let doc = JsonDoc::parse(r#"{"hooks": {"stop": [{"command": "x", "timeout": 5}]}}"#);
        assert!(!SettingsView::build(&doc).copilot_hooks);
    }
}
