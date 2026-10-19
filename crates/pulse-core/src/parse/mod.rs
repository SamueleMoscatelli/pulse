//! Per-kind parsed views (spec 02-assets → Per-kind parsed views).

pub mod json;
pub mod markdown;
pub mod mcp;
pub mod settings;
pub mod tools;

pub use json::{JsonDoc, JsonMember, JsonNode, JsonValue, ViewError};
pub use markdown::{CodeBlock, Frontmatter, Heading, Import, Link, MarkdownView, Table, YamlError};
pub use mcp::{McpInput, McpServer, McpView, Transport};
pub use settings::{HookCmd, HookEntry, SettingsView};
pub use tools::{ToolDef, ToolsView};
