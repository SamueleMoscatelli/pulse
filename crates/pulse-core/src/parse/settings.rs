//! Settings / hooks view: Claude `settings.json` (`permissions`, `hooks`, `env`, flags) and
//! standalone hook files (Claude plugin `hooks/hooks.json`, Cursor `.cursor/hooks.json`).

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
    /// The `command`, or the `prompt` text of a prompt hook; empty for other types.
    pub command: String,
    /// The command text, or the whole hook object when there is none.
    pub command_span: Span,
    /// Seconds.
    pub timeout: Option<f64>,
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
                "hooks" => hooks(&mut sh, n, &mut v.hooks),
                _ => {}
            }
        }
        v.errors = sh.errors;
        v
    }
}

fn hooks(sh: &mut Shape, node: &JsonNode, out: &mut Vec<HookEntry>) {
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
                        } else if let Some(c) = cmd(sh, h) {
                            cmds.push(c);
                        }
                    }
                }
                // Cursor: `{ command }` directly under the event.
                None => cmds.extend(cmd(sh, group)),
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
}

fn cmd(sh: &mut Shape, node: &JsonNode) -> Option<HookCmd> {
    let kind = node
        .get("type")
        .and_then(|n| sh.string(n, "type"))
        .map_or_else(|| "command".to_owned(), |t| t.0);
    let text = node.get("command").or_else(|| node.get("prompt"));
    let (command, command_span) = match text {
        Some(t) => sh.string(t, "command")?,
        // Other hook types (`http`, `agent`, …) carry no command text.
        None if !matches!(kind.as_str(), "command" | "prompt") => (String::new(), node.span),
        None => {
            sh.errors.push(ViewError::new(
                node.span,
                format!("`{kind}` hook has no `{kind}`"),
            ));
            return None;
        }
    };
    let timeout = node.get("timeout").and_then(|n| sh.number(n, "timeout"));
    Some(HookCmd {
        kind,
        command,
        command_span,
        timeout,
    })
}
