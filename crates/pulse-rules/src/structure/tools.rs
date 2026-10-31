//! AI-STR-005: unknown tool in an allow-list (agent `tools`, command `allowed-tools`,
//! settings permissions). Project scope: `mcp__<server>__…` needs every McpConfig's servers.

use std::collections::BTreeSet;

use pulse_core::{
    Asset, AssetKind, Category, Finding, KindSet, ProjectRule, RuleCtx, RuleMeta, Severity, Span,
};

use crate::lexicon::platform::{
    CLAUDE_IMPLICIT_SERVERS, CLAUDE_TOOLS, COPILOT_TOOL_ALIASES, COPILOT_TOOLS,
};
use crate::security::mcp::value_span;
use crate::util::{distance_within, suggest, truncate};

pub static META: RuleMeta = RuleMeta {
    id: "AI-STR-005",
    name: "Unknown tool in allow-list",
    category: Category::Structure,
    default_severity: Severity::Medium,
    kinds: KindSet::AGENT
        .union(KindSet::PROMPT)
        .union(KindSet::SETTINGS),
    summary: "An allow-list names a tool that does not exist, so the grant silently does nothing.",
    remediation: "Fix the tool name, or define the MCP server it belongs to.",
    inline_suppressible: true,
};

/// Chars of an entry quoted in a message.
const QUOTED: usize = 40;

pub struct UnknownTool;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Platform {
    Claude,
    Copilot,
}

fn platform(asset: &Asset) -> Option<Platform> {
    let rel = asset.rel_path.as_str();
    // `*.agent.md` / `*.prompt.md` are Copilot formats wherever they live (P8.7).
    if rel.starts_with(".github/")
        || rel.contains("/.github/")
        || rel.ends_with(".agent.md")
        || rel.ends_with(".prompt.md")
    {
        Some(Platform::Copilot)
    } else if rel.ends_with(".prompty") {
        None
    } else {
        // Settings, `.claude/agents`, `.claude/commands`.
        Some(Platform::Claude)
    }
}

/// An MCP server name as Claude spells it in tool names.
fn tool_ident(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Servers of every McpConfig of the project; `None` when there is no McpConfig.
fn mcp_servers(assets: &[Asset]) -> Option<BTreeSet<String>> {
    let mut configs = assets
        .iter()
        .filter(|a| a.kind == AssetKind::McpConfig && a.skipped.is_none())
        .peekable();
    configs.peek()?;
    Some(
        configs
            .flat_map(|a| &a.mcp().servers)
            .map(|s| tool_ident(&s.name))
            .collect(),
    )
}

impl ProjectRule for UnknownTool {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_project(&self, assets: &[Asset], _ctx: &RuleCtx<'_>, out: &mut Vec<Finding>) {
        let servers = mcp_servers(assets);
        for asset in assets {
            if !META.kinds.contains(asset.kind) || asset.skipped.is_some() {
                continue;
            }
            let Some(platform) = platform(asset) else {
                continue;
            };
            let mut each = |entry: &str, span: Span| {
                let message = match platform {
                    Platform::Claude => claude_error(entry, servers.as_ref()),
                    Platform::Copilot => copilot_error(entry, servers.as_ref()),
                };
                if let Some(message) = message {
                    out.push(Finding::new(&META, asset, span, message));
                }
            };
            if asset.kind == AssetKind::Settings {
                let s = asset.settings();
                for (entry, span) in s.allow.iter().chain(&s.deny).chain(&s.ask) {
                    let name = entry_name(entry);
                    let off = entry.find(name).unwrap_or(0);
                    each(entry, value_span(asset, *span, entry, off, name.len()));
                }
            } else {
                frontmatter_entries(asset, &mut each);
            }
        }
    }
}

/// The tool of a permission entry: `Bash(npm test)` → `Bash`.
fn entry_name(entry: &str) -> &str {
    entry.split('(').next().unwrap_or("").trim()
}

fn claude_error(entry: &str, servers: Option<&BTreeSet<String>>) -> Option<String> {
    let name = entry_name(entry);
    if name.is_empty() || name == "*" || CLAUDE_TOOLS.contains(&name) {
        return None;
    }
    if let Some(rest) = name.strip_prefix("mcp__") {
        let server = rest.split("__").next().unwrap_or("");
        let implicit = server == "ide"
            || CLAUDE_IMPLICIT_SERVERS
                .iter()
                .any(|p| p.ends_with('_') && server.starts_with(p));
        // AI-STR-005 FP guard: without any project McpConfig, servers may be user-scoped.
        let servers = servers?;
        if server.is_empty() || server.contains('*') || implicit || servers.contains(server) {
            return None;
        }
        let known: Vec<&str> = servers.iter().map(String::as_str).collect();
        let hint =
            suggest(server, &known).map_or_else(String::new, |s| format!(" (did you mean `{s}`?)"));
        return Some(format!(
            "Undefined MCP server `{}` in tool name{hint}",
            truncate(server, QUOTED)
        ));
    }
    let hint =
        suggest(name, CLAUDE_TOOLS).map_or_else(String::new, |s| format!(" (did you mean `{s}`?)"));
    Some(format!("Unknown tool `{}`{hint}", truncate(name, QUOTED)))
}

fn copilot_error(entry: &str, servers: Option<&BTreeSet<String>>) -> Option<String> {
    let name = entry.trim();
    // Tool sets (`server/*`, `edit/editFiles`) and MCP servers by name are not checked.
    let known = |list: &[&str]| list.iter().any(|t| t.eq_ignore_ascii_case(name));
    if name.contains(['/', '.', '*'])
        || known(COPILOT_TOOLS)
        || known(COPILOT_TOOL_ALIASES)
        || servers.is_some_and(|s| s.contains(name))
    {
        return None;
    }
    // AI-STR-005 FP guard: extensions add tools, so only one-edit typos are reported.
    let s = suggest(name, COPILOT_TOOLS).filter(|s| distance_within(name, s, 1).is_some())?;
    Some(format!(
        "Unknown tool `{}` (did you mean `{s}`?)",
        truncate(name, QUOTED)
    ))
}

/// Splits `Read, Bash(git add:*) Grep` on commas and blanks outside parentheses.
fn split_entries(list: &str) -> Vec<&str> {
    let (mut out, mut depth, mut start) = (Vec::new(), 0u32, 0);
    for (i, c) in list.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' | ' ' | '\t' if depth == 0 => {
                out.push(&list[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&list[start..]);
    out.retain(|e| !e.trim().is_empty());
    out
}

/// Entries of the `tools` / `allowed-tools` frontmatter keys, each with its span.
fn frontmatter_entries(asset: &Asset, f: &mut impl FnMut(&str, Span)) {
    let Some(fm) = &asset.markdown().frontmatter else {
        return;
    };
    for key in ["tools", "allowed-tools"] {
        let Some(value) = fm.get(key) else {
            continue;
        };
        let entries: Vec<&str> = match (value.as_str(), value.as_sequence()) {
            (Some(s), _) => split_entries(s),
            (None, Some(seq)) => seq.iter().filter_map(|v| v.as_str()).collect(),
            _ => continue,
        };
        let Some(&line) = fm.key_lines.get(key) else {
            continue;
        };
        let line_span = asset.lines.line_span(&asset.source, line);
        let mut from = line_span.start.max(fm.body_span.start) as usize;
        for entry in entries {
            let entry = entry.trim();
            let span = locate(asset, from, fm.body_span.end as usize, entry).unwrap_or(line_span);
            if span != line_span {
                from = span.end as usize;
            }
            f(entry, span);
        }
    }
}

/// The first `entry` in `source[from..to]`, not inside a longer word.
fn locate(asset: &Asset, from: usize, to: usize, entry: &str) -> Option<Span> {
    let text = asset.source.get(from..to)?;
    let word = |b: Option<u8>| b.is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_');
    let mut pos = 0;
    while let Some(i) = text.get(pos..).and_then(|t| t.find(entry)) {
        let at = pos + i;
        let end = at + entry.len();
        if !word(text[..at].bytes().next_back()) && !word(text[end..].bytes().next()) {
            return Some(Span::new(
                u32::try_from(from + at).ok()?,
                u32::try_from(from + end).ok()?,
            ));
        }
        pos = end;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_entries_respects_parentheses() {
        assert_eq!(
            split_entries("Read, Bash(git add:*, git commit:*) Grep"),
            ["Read", "Bash(git add:*, git commit:*)", "Grep"]
        );
        assert_eq!(split_entries(""), Vec::<&str>::new());
    }

    #[test]
    fn claude_entries() {
        let servers: BTreeSet<String> = ["github".to_owned()].into();
        let err = |e| claude_error(e, Some(&servers));
        for ok in [
            "Read",
            "Bash(npm test)",
            "mcp__github__create_issue",
            "mcp__github",
            "mcp__ide__getDiagnostics",
            "mcp__claude_ai_Gmail__send",
            "*",
        ] {
            assert_eq!(err(ok), None, "{ok}");
        }
        assert_eq!(
            err("Reed").as_deref(),
            Some("Unknown tool `Reed` (did you mean `Read`?)")
        );
        assert_eq!(
            err("mcp__githb__x").as_deref(),
            Some("Undefined MCP server `githb` in tool name (did you mean `github`?)")
        );
        assert_eq!(claude_error("mcp__jira__x", None), None);
    }

    #[test]
    fn copilot_reports_only_typos() {
        assert_eq!(copilot_error("codebase", None), None);
        assert_eq!(copilot_error("myExtensionTool", None), None);
        assert_eq!(copilot_error("github/*", None), None);
        assert!(copilot_error("codebas", None).is_some());
    }

    #[test]
    fn server_names_become_tool_idents() {
        assert_eq!(tool_ident("my.server name"), "my_server_name");
    }
}
