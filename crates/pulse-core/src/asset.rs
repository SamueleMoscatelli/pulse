//! A discovered AI asset: its source text plus lazily parsed views.

use std::path::Path;
use std::sync::{Arc, OnceLock};

use serde_yaml_ng::Value;

use crate::model::{AssetKind, KindSet, LoadMode, Location, Span};
use crate::parse::{JsonDoc, MarkdownView, McpView, SettingsView, ToolsView};
use crate::text::{LineIndex, Normalized, normalize, tokens};

/// One file under analysis. Owns its source; parsed views are computed at most once.
#[derive(Debug)]
pub struct Asset {
    pub path: Arc<Path>,
    /// Path relative to the scan root, always with `/` separators.
    pub rel_path: String,
    pub kind: AssetKind,
    pub load: LoadMode,
    pub source: String,
    pub lines: LineIndex,
    /// The file was not valid UTF-8 and `source` is its lossy decoding (AI-STR-002).
    pub lossy: bool,
    /// Why the file was not analyzed (over `scan.max_file_bytes`, unreadable); `source` is
    /// then empty and only AI-STR-002 runs on it.
    pub skipped: Option<String>,
    /// `context.calibration` applied by [`Asset::tokens`].
    calibration: f64,
    markdown: OnceLock<MarkdownView>,
    json: OnceLock<JsonDoc>,
    mcp: OnceLock<McpView>,
    settings: OnceLock<SettingsView>,
    tools_view: OnceLock<ToolsView>,
    normalized: OnceLock<Normalized>,
    tokens: OnceLock<u32>,
}

impl Asset {
    pub fn new(
        path: impl Into<Arc<Path>>,
        rel_path: impl Into<String>,
        kind: AssetKind,
        load: LoadMode,
        source: String,
    ) -> Self {
        let mut rel_path = rel_path.into();
        if rel_path.contains('\\') {
            rel_path = rel_path.replace('\\', "/");
        }
        let lines = LineIndex::new(&source);
        Self {
            path: path.into(),
            rel_path,
            kind,
            load,
            source,
            lines,
            lossy: false,
            skipped: None,
            calibration: 1.0,
            markdown: OnceLock::new(),
            json: OnceLock::new(),
            mcp: OnceLock::new(),
            settings: OnceLock::new(),
            tools_view: OnceLock::new(),
            normalized: OnceLock::new(),
            tokens: OnceLock::new(),
        }
    }

    /// Like [`Asset::new`] from raw bytes: invalid UTF-8 is decoded lossily and flagged.
    pub fn from_bytes(
        path: impl Into<Arc<Path>>,
        rel_path: impl Into<String>,
        kind: AssetKind,
        load: LoadMode,
        bytes: Vec<u8>,
    ) -> Self {
        let (source, lossy) = match String::from_utf8(bytes) {
            Ok(s) => (s, false),
            Err(e) => (String::from_utf8_lossy(e.as_bytes()).into_owned(), true),
        };
        let mut asset = Self::new(path, rel_path, kind, load, source);
        asset.lossy = lossy;
        asset
    }

    /// An empty asset standing for a file that was not analyzed, with the reason.
    pub fn skipped(
        path: impl Into<Arc<Path>>,
        rel_path: impl Into<String>,
        kind: AssetKind,
        load: LoadMode,
        reason: impl Into<String>,
    ) -> Self {
        let mut asset = Self::new(path, rel_path, kind, load, String::new());
        asset.skipped = Some(reason.into());
        asset
    }

    /// Sets the token calibration factor (`context.calibration`, default 1.0).
    #[must_use]
    pub fn with_calibration(mut self, calibration: f64) -> Self {
        self.calibration = calibration;
        self.tokens = OnceLock::new();
        self
    }

    /// The Markdown view, parsed on first use. Any kind can be viewed; rules ask only for
    /// Markdown kinds (plain-text `.cursorrules` included).
    pub fn markdown(&self) -> &MarkdownView {
        self.markdown
            .get_or_init(|| MarkdownView::parse(&self.source, &self.lines))
    }

    /// The generic JSON(C) document, parsed on first use.
    pub fn json(&self) -> &JsonDoc {
        self.json.get_or_init(|| JsonDoc::parse(&self.source))
    }

    /// The MCP config view (McpConfig assets), built on first use.
    pub fn mcp(&self) -> &McpView {
        self.mcp.get_or_init(|| McpView::build(self.json()))
    }

    /// The settings view (Settings assets; a Hooks asset fills only `hooks`), built on first use.
    pub fn settings(&self) -> &SettingsView {
        self.settings
            .get_or_init(|| SettingsView::build(self.json()))
    }

    /// The hook entries of a Settings or Hooks asset.
    pub fn hooks(&self) -> &[crate::parse::HookEntry] {
        &self.settings().hooks
    }

    /// The tool definitions view (Tools assets), built on first use.
    pub fn tools(&self) -> &ToolsView {
        self.tools_view
            .get_or_init(|| ToolsView::build(self.json()))
    }

    /// The normalized source (spec "Text normalization"), computed on first use.
    pub fn normalized(&self) -> &Normalized {
        self.normalized.get_or_init(|| normalize(&self.source))
    }

    /// Estimated tokens of the whole file; Markdown kinds count fenced code at the code rate.
    pub fn tokens(&self) -> u32 {
        *self.tokens.get_or_init(|| {
            if KindSet::MARKDOWN.contains(self.kind) {
                let code: Vec<Span> = self.markdown().code_spans().collect();
                tokens::estimate_markdown(&self.source, &code, self.calibration)
            } else {
                tokens::estimate(&self.source, self.calibration)
            }
        })
    }

    /// Tokens this asset adds to every session (spec 00-overview → Always-on context): the
    /// whole file for always-loaded instructions, prompts and tools; only the frontmatter
    /// `description` for skills and agents; nothing for MCP configs, settings and hooks.
    pub fn always_tokens(&self) -> u32 {
        if self.effective_load() != LoadMode::Always {
            return 0;
        }
        match self.kind {
            AssetKind::Skill | AssetKind::Agent => self
                .markdown()
                .frontmatter
                .as_ref()
                .and_then(|f| f.get("description"))
                .and_then(Value::as_str)
                .map_or(0, |d| tokens::estimate(d, self.calibration)),
            AssetKind::McpConfig | AssetKind::Settings | AssetKind::Hooks => 0,
            _ => self.tokens(),
        }
    }

    /// Load mode after frontmatter promotion (spec 02-assets → Discovery table, footnote *):
    /// a Conditional ScopedInstructions becomes Always with `.mdc` `alwaysApply: true`, or for
    /// `.claude/rules` / `.github/instructions` without `paths`/`applyTo` or with `applyTo: "**"`.
    /// Invalid YAML keeps the discovered mode.
    pub fn effective_load(&self) -> LoadMode {
        if self.kind != AssetKind::ScopedInstructions || self.load != LoadMode::Conditional {
            return self.load;
        }
        let fm = self.markdown().frontmatter.as_ref();
        if fm.is_some_and(|f| f.value.is_err()) {
            return self.load;
        }
        let get = |key: &str| fm.and_then(|f| f.get(key));
        let promoted = if self.rel_path.ends_with(".mdc") {
            get("alwaysApply").and_then(Value::as_bool) == Some(true)
        } else if in_dir(&self.rel_path, ".claude/rules/")
            || in_dir(&self.rel_path, ".github/instructions/")
        {
            match (get("paths"), get("applyTo")) {
                (None, None) => true,
                (_, Some(apply)) => apply.as_str().map(str::trim) == Some("**"),
                (Some(_), None) => false,
            }
        } else {
            false
        };
        if promoted {
            LoadMode::Always
        } else {
            self.load
        }
    }

    /// Resolves a byte span to a 1-based, char-column location in this asset.
    pub fn span_to_location(&self, span: Span) -> Location {
        let (line, col) = self.lines.location(&self.source, span.start);
        let (end_line, end_col) = self.lines.location(&self.source, span.end.max(span.start));
        Location {
            path: Arc::clone(&self.path),
            line,
            col,
            end_line,
            end_col,
        }
    }
}

/// Whether `rel_path` lies under a `dir` (ending in `/`) at any depth.
fn in_dir(rel_path: &str, dir: &str) -> bool {
    rel_path.starts_with(dir)
        || rel_path
            .match_indices(dir)
            .any(|(i, _)| rel_path.as_bytes()[i - 1] == b'/')
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn span_to_location_and_rel_path() {
        let path: Arc<Path> = PathBuf::from("/r/a/CLAUDE.md").into();
        let asset = Asset::new(
            path,
            "a\\CLAUDE.md",
            AssetKind::Instructions,
            LoadMode::Always,
            "# T\nhello é world\n".to_owned(),
        );
        assert_eq!(asset.rel_path, "a/CLAUDE.md");
        let loc = asset.span_to_location(Span::new(4, 13));
        assert_eq!((loc.line, loc.col, loc.end_line, loc.end_col), (2, 1, 2, 9));
    }

    fn asset(rel: &str, kind: AssetKind, load: LoadMode, src: &str) -> Asset {
        Asset::new(PathBuf::from(rel), rel, kind, load, src.to_owned())
    }

    fn scoped(rel: &str, src: &str) -> LoadMode {
        asset(
            rel,
            AssetKind::ScopedInstructions,
            LoadMode::Conditional,
            src,
        )
        .effective_load()
    }

    #[test]
    fn effective_load_promotes_scoped_from_frontmatter() {
        use LoadMode::{Always, Conditional};
        let mdc = ".cursor/rules/a.mdc";
        assert_eq!(scoped(mdc, "---\nalwaysApply: true\n---\nx"), Always);
        assert_eq!(
            scoped(mdc, "---\nalwaysApply: false\nglobs: '*.ts'\n---\n"),
            Conditional
        );
        assert_eq!(scoped(mdc, "no frontmatter"), Conditional);
        let claude = "pkg/.claude/rules/r.md";
        assert_eq!(scoped(claude, "plain rule"), Always);
        assert_eq!(scoped(claude, "---\npaths: ['src/**']\n---\n"), Conditional);
        let gh = ".github/instructions/x.instructions.md";
        assert_eq!(scoped(gh, "---\napplyTo: \"**\"\n---\n"), Always);
        assert_eq!(scoped(gh, "---\napplyTo: \"**/*.ts\"\n---\n"), Conditional);
        assert_eq!(
            scoped(gh, "---\napplyTo: [\n---\n"),
            Conditional,
            "invalid YAML"
        );
        assert_eq!(scoped(".windsurf/rules/w.md", "x"), Conditional);
        let other = asset(
            mdc,
            AssetKind::Instructions,
            Conditional,
            "---\nalwaysApply: true\n---\n",
        );
        assert_eq!(other.effective_load(), Conditional);
    }

    #[test]
    fn from_bytes_flags_invalid_utf8() {
        let ok = Asset::from_bytes(
            PathBuf::from("a.md"),
            "a.md",
            AssetKind::Prompt,
            LoadMode::OnDemand,
            b"hi".to_vec(),
        );
        assert!(!ok.lossy);
        let bad = Asset::from_bytes(
            PathBuf::from("a.md"),
            "a.md",
            AssetKind::Prompt,
            LoadMode::OnDemand,
            b"h\xffi".to_vec(),
        );
        assert!(bad.lossy);
        assert_eq!(bad.source, "h\u{FFFD}i");
        let skipped = Asset::skipped(
            PathBuf::from("a.md"),
            "a.md",
            AssetKind::Prompt,
            LoadMode::OnDemand,
            "too big",
        );
        assert_eq!(
            (skipped.source.as_str(), skipped.skipped.as_deref()),
            ("", Some("too big"))
        );
    }

    #[test]
    fn always_tokens_by_kind() {
        let skill = asset(
            "s/SKILL.md",
            AssetKind::Skill,
            LoadMode::Always,
            "---\ndescription: one two three four\n---\nlong body text here and more\n",
        );
        assert_eq!(
            skill.always_tokens(),
            tokens::estimate("one two three four", 1.0)
        );
        let root = asset(
            "CLAUDE.md",
            AssetKind::Instructions,
            LoadMode::Always,
            "Use tabs.\n",
        );
        assert_eq!(root.always_tokens(), root.tokens());
        let prompt = asset(
            "p.prompt.md",
            AssetKind::Prompt,
            LoadMode::OnDemand,
            "Use tabs.\n",
        );
        assert_eq!(prompt.always_tokens(), 0);
        let mcp = asset(".mcp.json", AssetKind::McpConfig, LoadMode::Always, "{}");
        assert_eq!(mcp.always_tokens(), 0);
    }

    #[test]
    fn lazy_views_are_cached_and_cursorrules_is_markdown() {
        let a = asset(
            ".cursorrules",
            AssetKind::Instructions,
            LoadMode::Always,
            "Use tabs. See @docs/style.md\n",
        )
        .with_calibration(2.0);
        assert!(std::ptr::eq(a.markdown(), a.markdown()));
        assert_eq!(a.markdown().imports[0].path, "docs/style.md");
        assert_eq!(a.normalized().text, "use tabs. see @docs/style.md ");
        let raw = tokens::estimate(&a.source, 2.0);
        assert_eq!(a.tokens(), raw);
        assert_eq!(a.tokens(), raw);
    }
}
