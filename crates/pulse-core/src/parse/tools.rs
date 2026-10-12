//! Tool definitions view: Anthropic (`{name, description, input_schema}`) and OpenAI
//! (`{type: "function", function: {name, description, parameters}}` or the flat Responses form)
//! tools, as a root array or under a root `tools` key.

use crate::model::Span;
use crate::parse::json::{JsonDoc, JsonNode, Shape, ViewError};

/// Parsed tool definitions.
#[derive(Debug, Clone, Default)]
pub struct ToolsView {
    pub tools: Vec<ToolDef>,
    pub errors: Vec<ViewError>,
}

/// One tool definition.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolDef {
    pub name: String,
    pub name_span: Span,
    pub description: Option<String>,
    pub desc_span: Option<Span>,
    /// `input_schema` / `parameters` value.
    pub schema_span: Option<Span>,
    /// The whole definition object.
    pub span: Span,
}

impl ToolsView {
    pub fn build(doc: &JsonDoc) -> Self {
        let mut v = Self::default();
        let mut sh = Shape::default();
        let Some(root) = &doc.root else {
            return v;
        };
        let list = match root.get("tools") {
            Some(t) => t,
            None => root,
        };
        for item in sh.array(list, "tools") {
            if let Some(t) = tool(&mut sh, item) {
                v.tools.push(t);
            }
        }
        v.errors = sh.errors;
        v
    }
}

fn tool(sh: &mut Shape, item: &JsonNode) -> Option<ToolDef> {
    if item.as_object().is_none() {
        sh.object(item, "tools[]");
        return None;
    }
    // OpenAI Chat Completions nests the definition under `function`.
    let def = match item.get("function") {
        Some(f) if f.as_object().is_some() => f,
        _ => item,
    };
    let Some(name_node) = def.get("name") else {
        sh.errors
            .push(ViewError::new(item.span, "tool has no `name`"));
        return None;
    };
    let (name, name_span) = sh.string(name_node, "name")?;
    let desc = def
        .get("description")
        .and_then(|n| sh.string(n, "description"));
    let schema = def.get("input_schema").or_else(|| def.get("parameters"));
    if let Some(s) = schema {
        sh.object(s, "input_schema");
    }
    Some(ToolDef {
        name,
        name_span,
        desc_span: desc.as_ref().map(|d| d.1),
        description: desc.map(|d| d.0),
        schema_span: schema.map(|s| s.span),
        span: item.span,
    })
}
