//! Generic JSONC document: an owned, span-carrying tree plus JSON-pointer → span maps (spec
//! 02-assets → Per-kind parsed views). Comments and trailing commas are accepted. A syntax
//! error is recorded in [`JsonDoc::errors`], never raised.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::OnceLock;

use jsonc_parser::ast::{ObjectPropName, Value};
use jsonc_parser::{CollectOptions, ParseOptions, parse_to_ast};

use crate::model::Span;

/// A parsed JSON(C) file.
#[derive(Debug, Clone, Default)]
pub struct JsonDoc {
    /// `None` for an empty file or a syntax error.
    pub root: Option<JsonNode>,
    /// Pointer maps, built on first use: the typed views carry their own spans.
    maps: OnceLock<PointerMaps>,
    /// Syntax errors (at most one: parsing stops at the first).
    pub errors: Vec<ViewError>,
}

/// RFC 6901 pointer → spans of values and member keys.
#[derive(Debug, Clone, Default)]
pub struct PointerMaps {
    /// Pointer (`""` is the root) → span of the value.
    pub values: HashMap<String, Span>,
    /// Pointer of an object member → span of its key, quotes included.
    pub keys: HashMap<String, Span>,
}

/// A JSON value with the byte span it covers in the source.
#[derive(Debug, Clone, PartialEq)]
pub struct JsonNode {
    pub span: Span,
    pub value: JsonValue,
}

#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    /// The number as written.
    Number(String),
    String(String),
    Array(Vec<JsonNode>),
    Object(Vec<JsonMember>),
}

/// One `"key": value` pair, in source order (duplicates kept).
#[derive(Debug, Clone, PartialEq)]
pub struct JsonMember {
    pub key: String,
    pub key_span: Span,
    pub value: JsonNode,
}

/// A syntax or shape problem found while building a view (consumed by AI-STR-002).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewError {
    pub span: Span,
    pub msg: String,
}

impl ViewError {
    pub fn new(span: Span, msg: impl Into<String>) -> Self {
        Self {
            span,
            msg: msg.into(),
        }
    }
}

impl JsonDoc {
    pub fn parse(source: &str) -> Self {
        let mut doc = Self::default();
        match parse_to_ast(source, &CollectOptions::default(), &ParseOptions::default()) {
            Ok(res) => {
                doc.root = res.value.map(convert);
            }
            Err(e) => {
                let r = e.range();
                doc.errors.push(ViewError::new(
                    Span::new(to_u32(r.start), to_u32(r.end)),
                    e.kind().to_string(),
                ));
            }
        }
        doc
    }

    /// Every value and key pointer of the document, indexed on first use.
    pub fn pointer_spans(&self) -> &PointerMaps {
        self.maps.get_or_init(|| {
            let mut maps = PointerMaps::default();
            if let Some(root) = &self.root {
                index(root, &mut String::new(), &mut maps);
            }
            maps
        })
    }

    /// Span of the value at `pointer` (RFC 6901, e.g. `/mcpServers/db/env/TOKEN`).
    pub fn span_of(&self, pointer: &str) -> Option<Span> {
        self.pointer(pointer).map(|n| n.span)
    }

    /// Span of the key of the member at `pointer`.
    pub fn key_span_of(&self, pointer: &str) -> Option<Span> {
        let (parent, last) = pointer.rsplit_once('/')?;
        let member = self.pointer(parent)?.member(&unescape(last))?;
        Some(member.key_span)
    }

    /// The node at `pointer`.
    pub fn pointer(&self, pointer: &str) -> Option<&JsonNode> {
        let mut node = self.root.as_ref()?;
        if pointer.is_empty() {
            return Some(node);
        }
        for raw in pointer.strip_prefix('/')?.split('/') {
            let token = unescape(raw);
            node = match &node.value {
                JsonValue::Object(_) => node.get(&token)?,
                JsonValue::Array(items) => items.get(token.parse::<usize>().ok()?)?,
                _ => return None,
            };
        }
        Some(node)
    }
}

impl JsonNode {
    /// The member value for `key` (the last one wins, as in most JSON readers).
    pub fn get(&self, key: &str) -> Option<&JsonNode> {
        self.member(key).map(|m| &m.value)
    }

    /// The member for `key` (the last one wins).
    pub fn member(&self, key: &str) -> Option<&JsonMember> {
        self.as_object()?.iter().rev().find(|m| m.key == key)
    }

    pub fn as_object(&self) -> Option<&[JsonMember]> {
        match &self.value {
            JsonValue::Object(m) => Some(m),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[JsonNode]> {
        match &self.value {
            JsonValue::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match &self.value {
            JsonValue::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self.value {
            JsonValue::Bool(b) => Some(b),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match &self.value {
            JsonValue::Number(n) => n.parse().ok(),
            _ => None,
        }
    }

    /// JSON type name, for error messages.
    pub fn type_name(&self) -> &'static str {
        match self.value {
            JsonValue::Null => "null",
            JsonValue::Bool(_) => "boolean",
            JsonValue::Number(_) => "number",
            JsonValue::String(_) => "string",
            JsonValue::Array(_) => "array",
            JsonValue::Object(_) => "object",
        }
    }
}

/// Shape-error collector shared by the typed views.
#[derive(Debug, Default)]
pub(crate) struct Shape {
    pub errors: Vec<ViewError>,
}

impl Shape {
    fn expected(&mut self, node: &JsonNode, what: &str, want: &str) {
        self.errors.push(ViewError::new(
            node.span,
            format!("`{what}` must be {want}, found {}", node.type_name()),
        ));
    }

    /// The members of `node`, or an error when it is not an object.
    pub fn object<'n>(&mut self, node: &'n JsonNode, what: &str) -> &'n [JsonMember] {
        node.as_object().unwrap_or_else(|| {
            self.expected(node, what, "an object");
            &[]
        })
    }

    /// The items of `node`, or an error when it is not an array.
    pub fn array<'n>(&mut self, node: &'n JsonNode, what: &str) -> &'n [JsonNode] {
        node.as_array().unwrap_or_else(|| {
            self.expected(node, what, "an array");
            &[]
        })
    }

    /// The string in `node` with its span, or an error when it is not a string.
    pub fn string(&mut self, node: &JsonNode, what: &str) -> Option<(String, Span)> {
        let s = node.as_str().map(|s| (s.to_owned(), node.span));
        if s.is_none() {
            self.expected(node, what, "a string");
        }
        s
    }

    /// The string items of an array of strings; other items become errors.
    pub fn strings(&mut self, node: &JsonNode, what: &str) -> Vec<(String, Span)> {
        self.array(node, what)
            .iter()
            .filter_map(|n| self.string(n, what))
            .collect()
    }

    /// An object of string values as `(key, value, value span)`.
    pub fn string_map(&mut self, node: &JsonNode, what: &str) -> Vec<(String, String, Span)> {
        self.object(node, what)
            .iter()
            .filter_map(|m| {
                self.string(&m.value, what)
                    .map(|(v, s)| (m.key.clone(), v, s))
            })
            .collect()
    }

    pub fn bool(&mut self, node: &JsonNode, what: &str) -> Option<bool> {
        let b = node.as_bool();
        if b.is_none() {
            self.expected(node, what, "a boolean");
        }
        b
    }

    pub fn number(&mut self, node: &JsonNode, what: &str) -> Option<f64> {
        let n = node.as_f64();
        if n.is_none() {
            self.expected(node, what, "a number");
        }
        n
    }
}

/// Decodes one RFC 6901 reference token.
fn unescape(token: &str) -> Cow<'_, str> {
    if token.contains('~') {
        Cow::Owned(token.replace("~1", "/").replace("~0", "~"))
    } else {
        Cow::Borrowed(token)
    }
}

fn to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn span(r: jsonc_parser::common::Range) -> Span {
    Span::new(to_u32(r.start), to_u32(r.end))
}

fn convert(v: Value<'_>) -> JsonNode {
    match v {
        Value::NullKeyword(n) => JsonNode {
            span: span(n.range),
            value: JsonValue::Null,
        },
        Value::BooleanLit(b) => JsonNode {
            span: span(b.range),
            value: JsonValue::Bool(b.value),
        },
        Value::NumberLit(n) => JsonNode {
            span: span(n.range),
            value: JsonValue::Number(n.value.to_owned()),
        },
        Value::StringLit(s) => JsonNode {
            span: span(s.range),
            value: JsonValue::String(s.value.into_owned()),
        },
        Value::Array(a) => JsonNode {
            span: span(a.range),
            value: JsonValue::Array(a.elements.into_iter().map(convert).collect()),
        },
        Value::Object(o) => JsonNode {
            span: span(o.range),
            value: JsonValue::Object(
                o.properties
                    .into_iter()
                    .map(|p| {
                        let key_span = match &p.name {
                            ObjectPropName::String(s) => span(s.range),
                            ObjectPropName::Word(w) => span(w.range),
                        };
                        JsonMember {
                            key: p.name.into_string(),
                            key_span,
                            value: convert(p.value),
                        }
                    })
                    .collect(),
            ),
        },
    }
}

/// Fills the pointer maps; `ptr` is the current pointer, restored on return.
fn index(node: &JsonNode, ptr: &mut String, doc: &mut PointerMaps) {
    doc.values.insert(ptr.clone(), node.span);
    let len = ptr.len();
    match &node.value {
        JsonValue::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                ptr.push('/');
                ptr.push_str(&i.to_string());
                index(item, ptr, doc);
                ptr.truncate(len);
            }
        }
        JsonValue::Object(members) => {
            for m in members {
                ptr.push('/');
                for c in m.key.chars() {
                    match c {
                        '~' => ptr.push_str("~0"),
                        '/' => ptr.push_str("~1"),
                        c => ptr.push(c),
                    }
                }
                doc.keys.insert(ptr.clone(), m.key_span);
                index(&m.value, ptr, doc);
                ptr.truncate(len);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(src: &str, s: Span) -> &str {
        &src[s.start as usize..s.end as usize]
    }

    #[test]
    fn jsonc_with_comments_and_trailing_commas() {
        let src = "// top\n{\n  \"a/b\": { \"x~y\": [1, \"é\", true,], }, /* c */\n  k: null,\n}\n";
        let doc = JsonDoc::parse(src);
        assert!(doc.errors.is_empty(), "{:?}", doc.errors);
        assert_eq!(text(src, doc.span_of("/a~1b/x~0y/1").unwrap()), "\"é\"");
        assert_eq!(text(src, doc.key_span_of("/a~1b").unwrap()), "\"a/b\"");
        assert_eq!(text(src, doc.key_span_of("/k").unwrap()), "k");
        let maps = doc.pointer_spans();
        assert_eq!(maps.values.len(), 7);
        assert_eq!(
            maps.values["/a~1b/x~0y/1"],
            doc.span_of("/a~1b/x~0y/1").unwrap()
        );
        assert_eq!(
            maps.keys["/a~1b/x~0y"],
            doc.key_span_of("/a~1b/x~0y").unwrap()
        );
        assert_eq!(
            doc.pointer("/a~1b/x~0y/2").and_then(JsonNode::as_bool),
            Some(true)
        );
        assert_eq!(doc.pointer("/k").map(JsonNode::type_name), Some("null"));
        assert!(doc.pointer("/a~1b/x~0y/9").is_none());
        assert_eq!(doc.span_of(""), doc.root.as_ref().map(|r| r.span));
    }

    #[test]
    fn syntax_error_is_recorded_not_raised() {
        let doc = JsonDoc::parse("{\n  \"a\": [1, 2\n");
        assert!(doc.root.is_none());
        assert_eq!(doc.errors.len(), 1);
        assert!(JsonDoc::parse("").root.is_none());
    }

    #[test]
    fn duplicate_key_last_wins() {
        let doc = JsonDoc::parse(r#"{"a": 1, "a": 2}"#);
        let root = doc.root.as_ref().unwrap();
        assert_eq!(root.get("a").and_then(JsonNode::as_f64), Some(2.0));
        assert_eq!(root.as_object().map(<[_]>::len), Some(2));
    }
}
