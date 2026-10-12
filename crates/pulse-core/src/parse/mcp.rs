//! MCP config view: servers from `mcpServers` (Claude, Cursor, Gemini, Windsurf) or VS Code
//! `servers` + `inputs`.

use crate::model::Span;
use crate::parse::json::{JsonDoc, JsonNode, Shape, ViewError};

/// Parsed MCP configuration.
#[derive(Debug, Clone, Default)]
pub struct McpView {
    /// In source order.
    pub servers: Vec<McpServer>,
    /// VS Code `inputs` (prompted variables).
    pub inputs: Vec<McpInput>,
    pub errors: Vec<ViewError>,
}

/// One configured server.
#[derive(Debug, Clone, PartialEq)]
pub struct McpServer {
    pub name: String,
    pub name_span: Span,
    pub transport: Transport,
    pub args: Vec<(String, Span)>,
    /// `(key, value, value span)`.
    pub env: Vec<(String, String, Span)>,
    /// `(key, value, value span)`.
    pub headers: Vec<(String, String, Span)>,
    /// The whole server object.
    pub span: Span,
}

/// How the client reaches the server.
#[derive(Debug, Clone, PartialEq)]
pub enum Transport {
    Stdio {
        command: (String, Span),
    },
    Http {
        url: (String, Span),
    },
    Sse {
        url: (String, Span),
    },
    /// Neither `command` nor a URL, or an unrecognized `type`.
    Unknown,
}

/// A VS Code `inputs` entry.
#[derive(Debug, Clone, PartialEq)]
pub struct McpInput {
    pub id: String,
    pub id_span: Span,
    /// `password: true`.
    pub password: bool,
    pub span: Span,
}

impl McpView {
    pub fn build(doc: &JsonDoc) -> Self {
        let mut view = Self::default();
        let mut sh = Shape::default();
        if let Some(root) = &doc.root {
            let root_members = sh.object(root, "(root)");
            for m in root_members {
                match m.key.as_str() {
                    "mcpServers" | "servers" => {
                        for s in sh.object(&m.value, &m.key) {
                            if let Some(srv) = server(&mut sh, &s.key, s.key_span, &s.value) {
                                view.servers.push(srv);
                            }
                        }
                    }
                    "inputs" => {
                        for n in sh.array(&m.value, "inputs") {
                            if let Some(i) = input(&mut sh, n) {
                                view.inputs.push(i);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        view.errors = sh.errors;
        view
    }
}

fn server(sh: &mut Shape, name: &str, name_span: Span, node: &JsonNode) -> Option<McpServer> {
    if node.as_object().is_none() {
        sh.object(node, name);
        return None;
    }
    let get = |k: &str| node.get(k);
    let field = |sh: &mut Shape, k: &str| node.get(k).and_then(|n| sh.string(n, k));
    let kind = field(sh, "type").map(|(t, _)| t);
    let command = field(sh, "command");
    // Gemini spells streamable HTTP `httpUrl`, Windsurf uses `serverUrl`.
    let http_url = field(sh, "httpUrl");
    let url = field(sh, "url").or_else(|| field(sh, "serverUrl"));
    let transport = match (kind.as_deref(), command, http_url, url) {
        (Some("stdio") | None, Some(c), _, _) => Transport::Stdio { command: c },
        (Some("sse"), _, _, Some(u)) => Transport::Sse { url: u },
        (Some("http" | "streamable-http" | "streamableHttp") | None, _, Some(u), _)
        | (Some("http" | "streamable-http" | "streamableHttp") | None, _, None, Some(u)) => {
            Transport::Http { url: u }
        }
        _ => {
            if let Some(t) = &kind {
                sh.errors.push(ViewError::new(
                    node.span,
                    format!("server `{name}` of type `{t}` lacks its command or URL"),
                ));
            }
            Transport::Unknown
        }
    };
    let args = get("args")
        .map(|n| sh.strings(n, "args"))
        .unwrap_or_default();
    let env = get("env")
        .map(|n| sh.string_map(n, "env"))
        .unwrap_or_default();
    let headers = get("headers")
        .map(|n| sh.string_map(n, "headers"))
        .unwrap_or_default();
    Some(McpServer {
        name: name.to_owned(),
        name_span,
        transport,
        args,
        env,
        headers,
        span: node.span,
    })
}

fn input(sh: &mut Shape, node: &JsonNode) -> Option<McpInput> {
    sh.object(node, "inputs[]");
    let id_node = node.get("id")?;
    let (id, id_span) = sh.string(id_node, "id")?;
    let password = node
        .get("password")
        .and_then(|n| sh.bool(n, "password"))
        .unwrap_or(false);
    Some(McpInput {
        id,
        id_span,
        password,
        span: node.span,
    })
}
