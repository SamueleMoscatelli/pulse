//! AI-SEC-007 over-permissive agent permissions: settings allow-lists and modes, agent and
//! command `tools` frontmatter, permission-bypass flags, MCP auto-approve lists.

use memchr::memmem;
use pulse_core::{
    Asset, AssetKind, Category, Finding, KindSet, Rule, RuleCtx, RuleMeta, Severity, Span,
};

use crate::lexicon::permissions::{
    BYPASS_FLAGS, NETWORK_TOOLS, UNRESTRICTED_BASH, UNRESTRICTED_WEBFETCH, WRITE_EXEC_WORDS,
};
use crate::security::mcp::value_span;
use crate::util::truncate;

/// Chars of a permission or tool name quoted in a message.
const QUOTED: usize = 40;

pub static META: RuleMeta = RuleMeta {
    id: "AI-SEC-007",
    name: "Over-permissive agent permissions",
    category: Category::Security,
    default_severity: Severity::High,
    kinds: KindSet::ALL,
    summary: "Agent permissions allow any shell command, any URL, or skip approval prompts.",
    remediation: "Allow specific commands (Bash(git diff:*)) and domains (WebFetch(domain:x)).",
    inline_suppressible: true,
};

/// AI-SEC-007.
pub struct OverPermissive;

impl Rule for OverPermissive {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, asset: &Asset, _ctx: &RuleCtx<'_>, out: &mut Vec<Finding>) {
        match asset.kind {
            AssetKind::Settings => settings(asset, out),
            AssetKind::Agent | AssetKind::Prompt => frontmatter(asset, out),
            AssetKind::McpConfig => auto_approve(asset, out),
            _ => {}
        }
        bypass_flags(asset, out);
    }
}

fn finding(asset: &Asset, span: Span, severity: Severity, message: String) -> Finding {
    let mut f = Finding::new(&META, asset, span, message);
    f.severity = severity;
    f
}

fn settings(asset: &Asset, out: &mut Vec<Finding>) {
    let s = asset.settings();
    for (entry, span) in &s.allow {
        let message = if entry == "*" {
            "Wildcard permission \"*\" allows every tool".to_owned()
        } else if UNRESTRICTED_BASH.contains(&entry.as_str()) {
            format!("Unrestricted shell permission \"{entry}\"")
        } else if UNRESTRICTED_WEBFETCH.contains(&entry.as_str()) {
            format!("Unrestricted fetch permission \"{entry}\" (no domain)")
        } else {
            continue;
        };
        let span = value_span(asset, *span, entry, 0, entry.len());
        out.push(finding(asset, span, Severity::High, message));
    }
    if let Some((mode, span)) = &s.default_mode
        && mode == "bypassPermissions"
    {
        let span = value_span(asset, *span, mode, 0, mode.len());
        let message = "Permission prompts disabled (defaultMode \"bypassPermissions\")".to_owned();
        out.push(finding(asset, span, Severity::High, message));
    }
    if let Some((true, span)) = s.enable_all_project_mcp {
        let message = "Every project MCP server enabled without review".to_owned();
        out.push(finding(asset, span, Severity::Medium, message));
    }
}

/// Agent `tools` / command `allowed-tools` with unrestricted Bash.
fn frontmatter(asset: &Asset, out: &mut Vec<Finding>) {
    let Some(fm) = &asset.markdown().frontmatter else {
        return;
    };
    for key in ["tools", "allowed-tools"] {
        let Some(value) = fm.get(key) else {
            continue;
        };
        // `tools: Bash, Read` or a YAML list.
        let tools: Vec<&str> = match (value.as_str(), value.as_sequence()) {
            (Some(s), _) => s.split(',').map(str::trim).collect(),
            (None, Some(seq)) => seq
                .iter()
                .filter_map(|v| v.as_str())
                .map(str::trim)
                .collect(),
            _ => continue,
        };
        let Some(bash) = tools.iter().find(|t| UNRESTRICTED_BASH.contains(t)) else {
            continue;
        };
        let network = tools.iter().find(|t| NETWORK_TOOLS.contains(t));
        let severity = bash_severity(network.is_some());
        let message = match network {
            Some(net) => format!("Unrestricted Bash with {net} in `{key}` (exfiltration chain)"),
            None => format!("Unrestricted Bash in `{key}`"),
        };
        let span = frontmatter_span(asset, fm.key_lines.get(key).copied(), fm.body_span, bash);
        out.push(finding(asset, span, severity, message));
    }
}

/// Unrestricted Bash alone is Medium (subagents legitimately use it); with a network tool
/// it is an exfiltration chain, High.
pub fn bash_severity(with_network: bool) -> Severity {
    if with_network {
        Severity::High
    } else {
        Severity::Medium
    }
}

/// The first `bash` entry at or after the key's line, inside the frontmatter body; else the
/// key's line.
fn frontmatter_span(asset: &Asset, key_line: Option<u32>, body: Span, bash: &str) -> Span {
    let Some(line) = key_line else {
        return body;
    };
    let line_span = asset.lines.line_span(&asset.source, line);
    let from = line_span.start.max(body.start) as usize;
    let Some(text) = asset.source.get(from..body.end as usize) else {
        return line_span;
    };
    // Skip the key itself (`tools:`), then find `Bash` as a whole entry.
    let key_end = text.find(':').map_or(0, |i| i + 1);
    let mut pos = key_end;
    while let Some(i) = text.get(pos..).and_then(|t| t.find(bash)) {
        let at = pos + i;
        let end = at + bash.len();
        let before = text[..at].bytes().next_back();
        let after = text[end..].bytes().next();
        let boundary = |b: Option<u8>| b.is_none_or(|b| !b.is_ascii_alphanumeric() && b != b'(');
        if boundary(before) && boundary(after) {
            if let (Ok(s), Ok(e)) = (u32::try_from(from + at), u32::try_from(from + end)) {
                return Span::new(s, e);
            }
            break;
        }
        pos = end;
    }
    line_span
}

/// MCP `alwaysAllow` (Cline, Roo) / `autoApprove` (Kiro, Cline) lists with write/exec tools
/// or `*`. Disabled servers count too: the list applies as soon as the server is enabled.
fn auto_approve(asset: &Asset, out: &mut Vec<Finding>) {
    for server in &asset.mcp().servers {
        for (key, name, span) in &server.auto_approve {
            let message = if name == "*" {
                format!(
                    "Every MCP tool auto-approved (`{key}`, server \"{}\")",
                    truncate(&server.name, 24)
                )
            } else if write_or_exec(name) {
                format!(
                    "Write/exec MCP tool \"{}\" auto-approved (server \"{}\")",
                    truncate(name, QUOTED),
                    truncate(&server.name, 24)
                )
            } else {
                continue;
            };
            let span = value_span(asset, *span, name, 0, name.len());
            out.push(finding(asset, span, Severity::High, message));
        }
    }
}

/// Whether an MCP tool name (`write_file`, `executeCommand`, `git-push`) contains a
/// write/exec word.
pub fn write_or_exec(name: &str) -> bool {
    words(name).any(|w| WRITE_EXEC_WORDS.contains(&w.to_ascii_lowercase().as_str()))
}

/// Words of an identifier, split on non-alphanumerics and lower→upper camelCase changes.
fn words(name: &str) -> impl Iterator<Item = &str> {
    let b = name.as_bytes();
    let mut cuts = vec![0];
    for i in 1..b.len() {
        if b[i - 1].is_ascii_lowercase() && b[i].is_ascii_uppercase() {
            cuts.push(i);
        }
    }
    cuts.push(b.len());
    (0..cuts.len() - 1)
        .filter_map(move |i| name.get(cuts[i]..cuts[i + 1]))
        .flat_map(|chunk| chunk.split(|c: char| !c.is_ascii_alphanumeric()))
        .filter(|w| !w.is_empty())
}

/// `--dangerously-skip-permissions` / `--yolo` anywhere in the text.
fn bypass_flags(asset: &Asset, out: &mut Vec<Finding>) {
    let src = asset.source.as_bytes();
    if memchr::memchr(b'-', src).is_none() {
        return;
    }
    for flag in BYPASS_FLAGS {
        for at in memmem::find_iter(src, flag.as_bytes()) {
            let end = at + flag.len();
            let before = at.checked_sub(1).map(|i| src[i]);
            let after = src.get(end).copied();
            let word = |b: Option<u8>| b.is_some_and(|b| b.is_ascii_alphanumeric() || b == b'-');
            if word(before) || word(after) {
                continue;
            }
            let (Ok(s), Ok(e)) = (u32::try_from(at), u32::try_from(end)) else {
                continue;
            };
            let message = format!("Permission prompts disabled by `{flag}`");
            out.push(finding(asset, Span::new(s, e), Severity::High, message));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bash_severity_matrix_medium_alone_high_with_network() {
        assert_eq!(bash_severity(false), Severity::Medium);
        assert_eq!(bash_severity(true), Severity::High);
    }

    #[test]
    fn write_or_exec_splits_snake_kebab_and_camel_case() {
        for n in [
            "write_file",
            "executeCommand",
            "git-push",
            "create_issue",
            "run",
            "DeleteBranch",
        ] {
            assert!(write_or_exec(n), "{n}");
        }
        for n in [
            "read_file",
            "list_issues",
            "search_code",
            "getStatus",
            "fetch",
            "readdir",
            "settings_get",
        ] {
            assert!(!write_or_exec(n), "{n}");
        }
    }
}
