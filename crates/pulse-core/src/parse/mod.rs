//! Per-kind parsed views (spec 02-assets → Per-kind parsed views).

pub mod agent;
pub mod json;
pub mod markdown;
pub mod mcp;
pub mod settings;
pub mod tools;

pub use agent::{AgentView, PermissionRule};
pub use json::{JsonDoc, JsonMember, JsonNode, JsonValue, ViewError};
pub use markdown::{
    CodeBlock, FileRef, Frontmatter, Heading, Import, Link, MarkdownView, Table, YamlError,
};
pub use mcp::{McpInput, McpServer, McpView, Transport};
pub use settings::{HookCmd, HookEntry, HookFormat, SettingsView};
pub use tools::{ToolDef, ToolsView};
