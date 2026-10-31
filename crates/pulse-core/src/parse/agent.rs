//! Kiro custom agent view (ADR-053): `.kiro/agents/*.json` from the JSON document, and
//! `.kiro/agents/*.md` from the frontmatter (the body is the prompt). Inline `hooks` and
//! `mcpServers` of a JSON agent are read by the settings and MCP views.

use serde_yaml_ng::Value;

use crate::model::Span;
use crate::parse::json::{JsonDoc, JsonNode, Shape, ViewError};
use crate::parse::markdown::Frontmatter;
use crate::text::LineIndex;

/// The fields of a Kiro agent rules read, with their spans. A Markdown agent's spans are
/// its key lines, or the entry inside a list.
#[derive(Debug, Clone, Default)]
pub struct AgentView {
    /// Top-level keys with the span of the key (JSON) or of its line (Markdown).
    pub keys: Vec<(String, Span)>,
    pub name: Option<(String, Span)>,
    pub description: Option<(String, Span)>,
    /// The JSON `prompt`: inline text or a `file://` URI.
    pub prompt: Option<(String, Span)>,
    pub tools: Vec<(String, Span)>,
    /// CLI 2.x pre-approved tools.
    pub allowed_tools: Vec<(String, Span)>,
    /// Tools configured under the CLI 2.x `toolsSettings` (`allowedPaths`, `allowedCommands`).
    pub tools_settings: Vec<String>,
    /// `permissions.rules`.
    pub rules: Vec<PermissionRule>,
    /// String `resources` (`file://`, `skill://`); `knowledgeBase` objects are skipped.
    pub resources: Vec<(String, Span)>,
    pub include_mcp_json: Option<bool>,
    pub model: Option<(String, Span)>,
    pub errors: Vec<ViewError>,
}

/// One `permissions.rules` entry.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PermissionRule {
    pub capability: Option<String>,
    /// `match` globs; `None` when absent.
    pub matches: Option<Vec<String>>,
    pub effect: Option<String>,
    pub exclude: Vec<String>,
    /// The rule object (JSON) or the `permissions` key line (Markdown).
    pub span: Span,
}

impl AgentView {
    /// The span of top-level `key`, if present.
    pub fn key(&self, key: &str) -> Option<Span> {
        self.keys.iter().find(|(k, _)| k == key).map(|&(_, s)| s)
    }

    /// Whether `permissions` is set.
    pub fn has_permissions(&self) -> bool {
        self.key("permissions").is_some()
    }

    pub fn build(doc: &JsonDoc) -> Self {
        let mut v = Self::default();
        let mut sh = Shape::default();
        let Some(root) = &doc.root else {
            return v;
        };
        for m in sh.object(root, "(root)") {
            v.keys.push((m.key.clone(), m.key_span));
            let n = &m.value;
            match m.key.as_str() {
                "name" => v.name = sh.string(n, "name"),
                "description" => v.description = sh.string(n, "description"),
                "prompt" => v.prompt = sh.string(n, "prompt"),
                "model" => v.model = sh.string(n, "model"),
                "tools" => v.tools = sh.strings(n, "tools"),
                "allowedTools" => v.allowed_tools = sh.strings(n, "allowedTools"),
                "toolsSettings" => {
                    v.tools_settings = sh
                        .object(n, "toolsSettings")
                        .iter()
                        .map(|t| t.key.clone())
                        .collect();
                }
                "resources" => {
                    v.resources = sh
                        .array(n, "resources")
                        .iter()
                        .filter_map(|r| r.as_str().map(|s| (s.to_owned(), r.span)))
                        .collect();
                }
                "includeMcpJson" => v.include_mcp_json = sh.bool(n, "includeMcpJson"),
                "permissions" => {
                    for p in sh.object(n, "permissions") {
                        if p.key == "rules" {
                            for r in sh.array(&p.value, "rules") {
                                v.rules.extend(json_rule(&mut sh, r));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        v.errors = sh.errors;
        v
    }

    /// The view of a Markdown agent's frontmatter; invalid YAML gives an empty view
    /// (AI-STR-002 reports it).
    pub fn from_frontmatter(fm: &Frontmatter, source: &str, lines: &LineIndex) -> Self {
        let mut v = Self::default();
        let Ok(map) = &fm.value else {
            return v;
        };
        for (key, value) in map {
            let Some(key) = key.as_str() else {
                continue;
            };
            let Some(&line) = fm.key_lines.get(key) else {
                continue;
            };
            let line_span = lines.line_span(source, line);
            v.keys.push((key.to_owned(), line_span));
            let scalar = || value.as_str().map(|s| (s.to_owned(), line_span));
            let list = || yaml_list(value, source, line_span, fm.body_span);
            match key {
                "name" => v.name = scalar(),
                "description" => v.description = scalar(),
                "model" => v.model = scalar(),
                "tools" => v.tools = list(),
                "allowedTools" => v.allowed_tools = list(),
                "resources" => v.resources = list(),
                "toolsSettings" => {
                    v.tools_settings = value
                        .as_mapping()
                        .into_iter()
                        .flat_map(|m| m.keys().filter_map(Value::as_str))
                        .map(str::to_owned)
                        .collect();
                }
                "includeMcpJson" => v.include_mcp_json = value.as_bool(),
                "permissions" => {
                    let rules = value.get("rules").and_then(Value::as_sequence);
                    v.rules = rules
                        .into_iter()
                        .flatten()
                        .map(|r| yaml_rule(r, line_span))
                        .collect();
                }
                _ => {}
            }
        }
        v
    }
}

fn json_rule(sh: &mut Shape, node: &JsonNode) -> Option<PermissionRule> {
    if node.as_object().is_none() {
        sh.object(node, "rules[]");
        return None;
    }
    let text = |sh: &mut Shape, key: &str| {
        node.get(key)
            .and_then(|n| sh.string(n, key))
            .map(|(s, _)| s)
    };
    let globs = |sh: &mut Shape, key: &str| {
        node.get(key)
            .map(|n| sh.strings(n, key).into_iter().map(|(s, _)| s).collect())
    };
    Some(PermissionRule {
        capability: text(sh, "capability"),
        effect: text(sh, "effect"),
        matches: globs(sh, "match"),
        exclude: globs(sh, "exclude").unwrap_or_default(),
        span: node.span,
    })
}

fn yaml_rule(rule: &Value, span: Span) -> PermissionRule {
    let text = |key: &str| rule.get(key).and_then(Value::as_str).map(str::to_owned);
    let globs = |key: &str| {
        rule.get(key).map(|v| match (v.as_str(), v.as_sequence()) {
            (Some(s), _) => vec![s.to_owned()],
            (None, Some(seq)) => seq
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect(),
            _ => Vec::new(),
        })
    };
    PermissionRule {
        capability: text("capability"),
        effect: text("effect"),
        matches: globs("match"),
        exclude: globs("exclude").unwrap_or_default(),
        span,
    }
}

/// The entries of a YAML list (or a comma-separated string), each located after its key's
/// line inside the frontmatter; an entry not found there gets the key's line.
fn yaml_list(value: &Value, source: &str, key_line: Span, body: Span) -> Vec<(String, Span)> {
    let entries: Vec<&str> = match (value.as_str(), value.as_sequence()) {
        (Some(s), _) => s
            .split(',')
            .map(str::trim)
            .filter(|e| !e.is_empty())
            .collect(),
        (None, Some(seq)) => seq
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .collect(),
        _ => return Vec::new(),
    };
    let mut from = key_line.start.max(body.start) as usize;
    // Skip the key itself (`tools:`).
    if let Some(colon) = source
        .get(from..key_line.end as usize)
        .and_then(|l| l.find(':'))
    {
        from += colon + 1;
    }
    entries
        .into_iter()
        .map(|e| {
            let span = locate(source, from, body.end as usize, e).unwrap_or(key_line);
            if span != key_line {
                from = span.end as usize;
            }
            (e.to_owned(), span)
        })
        .collect()
}

/// The first `entry` in `source[from..to]`, not inside a longer word.
fn locate(source: &str, from: usize, to: usize, entry: &str) -> Option<Span> {
    let text = source.get(from..to)?;
    let word = |b: Option<u8>| b.is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_');
    let mut pos = 0;
    while let Some(i) = text.get(pos..).and_then(|t| t.find(entry)) {
        let at = pos + i;
        let end = at + entry.len();
        if !entry.is_empty()
            && !word(text[..at].bytes().next_back())
            && !word(text[end..].bytes().next())
        {
            return Some(Span::new(
                u32::try_from(from + at).ok()?,
                u32::try_from(from + end).ok()?,
            ));
        }
        pos = end.max(at + 1);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::MarkdownView;

    fn text(src: &str, span: Span) -> &str {
        &src[span.start as usize..span.end as usize]
    }

    #[test]
    fn json_agent_fields() {
        let src = r#"{
  "name": "rev",
  "description": "Reviews code.",
  "prompt": "file://./prompts/rev.md",
  "tools": ["@builtin", "@git"],
  "allowedTools": ["fs_read", "shell"],
  "toolsSettings": { "shell": { "allowedCommands": ["git status"] } },
  "permissions": { "rules": [
    { "capability": "shell", "match": ["git *"], "effect": "allow" },
    { "capability": "all", "effect": "ask", "exclude": ["~/**"] },
    3
  ] },
  "resources": ["file://README.md", { "type": "knowledgeBase" }],
  "includeMcpJson": true,
  "model": "claude-sonnet-4"
}"#;
        let v = AgentView::build(&JsonDoc::parse(src));
        assert_eq!(v.name.as_ref().unwrap().0, "rev");
        assert_eq!(v.prompt.as_ref().unwrap().0, "file://./prompts/rev.md");
        let names = |l: &[(String, Span)]| l.iter().map(|e| e.0.clone()).collect::<Vec<_>>();
        assert_eq!(names(&v.tools), ["@builtin", "@git"]);
        assert_eq!(names(&v.allowed_tools), ["fs_read", "shell"]);
        assert_eq!(text(src, v.allowed_tools[1].1), "\"shell\"");
        assert_eq!(v.tools_settings, ["shell"]);
        assert_eq!(v.rules.len(), 2);
        assert_eq!(v.rules[0].matches, Some(vec!["git *".to_owned()]));
        assert_eq!(
            (v.rules[1].matches.as_ref(), v.rules[1].exclude.len()),
            (None, 1)
        );
        assert_eq!(names(&v.resources), ["file://README.md"]);
        assert_eq!(
            (v.include_mcp_json, v.has_permissions()),
            (Some(true), true)
        );
        assert_eq!(text(src, v.key("model").unwrap()), "\"model\"");
        // `rules[2]` is not an object.
        assert_eq!(v.errors.len(), 1, "{:?}", v.errors);
    }

    #[test]
    fn markdown_agent_fields() {
        let src = "---\nname: rev\ndescription: Reviews code.\ntools: [read, shell]\nallowedTools:\n  - read\n  - shell\npermissions:\n  rules:\n    - capability: shell\n      match: \"*\"\n      effect: allow\n---\nBody.\n";
        let lines = LineIndex::new(src);
        let md = MarkdownView::parse(src, &lines);
        let v = AgentView::from_frontmatter(md.frontmatter.as_ref().unwrap(), src, &lines);
        assert_eq!(v.description.as_ref().unwrap().0, "Reviews code.");
        assert_eq!(text(src, v.tools[1].1), "shell");
        let shell = v.allowed_tools[1].1;
        assert_eq!((text(src, shell), lines.line_of(shell.start)), ("shell", 7));
        assert_eq!(v.rules[0].matches, Some(vec!["*".to_owned()]));
        assert_eq!(text(src, v.rules[0].span), "permissions:");
        assert!(v.has_permissions() && v.prompt.is_none());
    }
}
