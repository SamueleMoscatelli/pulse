//! Per-kind parsed views (spec 02-assets → Per-kind parsed views).

pub mod markdown;

pub use markdown::{CodeBlock, Frontmatter, Heading, Import, Link, MarkdownView, YamlError};
