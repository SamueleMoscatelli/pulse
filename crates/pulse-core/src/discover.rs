//! Asset discovery: walk the scan root and classify files with the discovery table.

use std::collections::HashSet;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use globset::{Candidate, Glob, GlobBuilder, GlobSet, GlobSetBuilder};
use ignore::{DirEntry, WalkBuilder, WalkState};

use crate::config::Config;
use crate::model::{AssetKind, LoadMode, Origin};
use crate::parse::json::{JsonDoc, JsonValue};
use crate::placement::{Misplaced, near_miss};

/// Directory names pruned at any depth (spec 02-assets → Default excludes).
pub const DEFAULT_EXCLUDES: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    "out",
    "vendor",
    ".venv",
    "venv",
    "__pycache__",
    ".next",
    ".turbo",
    "coverage",
];

/// Ignore file honored alongside `.gitignore` and `.ignore`.
pub const PULSE_IGNORE: &str = ".pulseignore";

/// Bytes read when sniffing a content-sniffed candidate (`tools/*.json`, `hooks.json`, `mcp.json`).
const SNIFF_BYTES: u64 = 64 * 1024;

/// Error building the classifier or starting the walk.
#[derive(Debug, thiserror::Error)]
pub enum DiscoverError {
    #[error("invalid glob `{glob}` in {field}: {source}")]
    Glob {
        field: &'static str,
        glob: String,
        source: globset::Error,
    },
    #[error("scan root {} is not a directory", .0.display())]
    NotADirectory(PathBuf),
}

/// A file classified as an asset. Contents are read later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredFile {
    pub path: Arc<Path>,
    /// Path relative to the scan root, always with `/` separators.
    pub rel_path: String,
    pub kind: AssetKind,
    pub load: LoadMode,
    pub origin: Origin,
}

/// Result of a walk. `assets` and `skipped` are sorted by path.
#[derive(Debug, Default)]
pub struct Discovery {
    pub assets: Vec<DiscoveredFile>,
    /// Rel paths of every walked (non-excluded) file, assets or not; used by AI-STR-003.
    pub all_paths: HashSet<String>,
    /// Files or directories that could not be walked or sniffed, with the reason.
    pub skipped: Vec<(PathBuf, String)>,
    /// Non-asset files with a near-miss name or folder, outside `scan.library` (AI-STR-008).
    pub misplaced: Vec<Misplaced>,
}

/// Post-match refinement attached to a table entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Refine {
    None,
    /// CLAUDE.md / AGENTS.md: root-level is Always, nested is Conditional.
    NestedConditional,
    /// Promoted to Always by frontmatter (see `Asset::effective_load`).
    Scoped,
    /// An asset only if its content has the expected shape.
    Sniff(Sniff),
}

/// Content test for files whose name alone is ambiguous.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sniff {
    /// `tools/*.json`: tool schemas.
    Tools,
    /// `hooks.json` anywhere: a top-level `hooks` object.
    Hooks,
    /// `mcp.json` anywhere: a top-level `mcpServers` or `servers` object.
    Mcp,
}

#[derive(Debug, Clone, Copy)]
struct Entry {
    kind: AssetKind,
    load: LoadMode,
    refine: Refine,
    /// Platform or library row, whose load mode `scan.library` turns into OnDemand, or
    /// custom.
    origin: Origin,
}

/// The built-in discovery table, in priority order (first match wins).
const TABLE: &[(&[&str], AssetKind, Refine)] = &[
    (
        &["**/.claude/settings.json", "**/.claude/settings.local.json"],
        AssetKind::Settings,
        Refine::None,
    ),
    (
        &["**/.cursor/hooks.json", "**/hooks/hooks.json"],
        AssetKind::Hooks,
        Refine::None,
    ),
    (
        &[
            "**/.mcp.json",
            "**/.vscode/mcp.json",
            "**/.cursor/mcp.json",
            "**/claude_desktop_config.json",
            "**/.gemini/settings.json",
            "**/.kiro/settings/mcp.json",
        ],
        AssetKind::McpConfig,
        Refine::None,
    ),
    (&["**/SKILL.md"], AssetKind::Skill, Refine::None),
    (
        &[
            "**/.claude/agents/**/*.md",
            "**/.github/agents/*.agent.md",
            "**/.github/chatmodes/*.chatmode.md",
        ],
        AssetKind::Agent,
        Refine::None,
    ),
    (
        &[
            "**/.claude/commands/**/*.md",
            "**/.github/prompts/*.prompt.md",
            "**/*.prompty",
        ],
        AssetKind::Prompt,
        Refine::None,
    ),
    (
        &[
            "**/.cursor/rules/**/*.mdc",
            "**/.claude/rules/**/*.md",
            "**/.github/instructions/**/*.instructions.md",
            "**/.windsurf/rules/**/*.md",
            "**/.clinerules/**/*.md",
            "**/.kiro/steering/**/*.md",
        ],
        AssetKind::ScopedInstructions,
        Refine::Scoped,
    ),
    (
        &["**/CLAUDE.md", "**/AGENTS.md"],
        AssetKind::Instructions,
        Refine::NestedConditional,
    ),
    (
        // `.clinerules` as a file lands here; as a directory, its `.md` files match row 7.
        &[
            "**/CLAUDE.local.md",
            "**/GEMINI.md",
            "**/.github/copilot-instructions.md",
            "**/.cursorrules",
            "**/.windsurfrules",
            "**/.clinerules",
        ],
        AssetKind::Instructions,
        Refine::None,
    ),
    (&["**/*.tools.json"], AssetKind::Tools, Refine::None),
    (
        &["**/tools/*.json"],
        AssetKind::Tools,
        Refine::Sniff(Sniff::Tools),
    ),
];

/// Library rows, matched after [`TABLE`]: assets stored for distribution, not loaded from
/// where they are, so they are always OnDemand (spec 02-assets → Discovery table, row 10).
const LIBRARY_TABLE: &[(&[&str], AssetKind, Refine)] = &[
    (
        &["**/*.agent.md", "**/*.chatmode.md"],
        AssetKind::Agent,
        Refine::None,
    ),
    (&["**/*.prompt.md"], AssetKind::Prompt, Refine::None),
    (
        &["**/*.instructions.md"],
        AssetKind::ScopedInstructions,
        Refine::None,
    ),
    (
        &["**/hooks.json"],
        AssetKind::Hooks,
        Refine::Sniff(Sniff::Hooks),
    ),
    (
        &["**/mcp.json"],
        AssetKind::McpConfig,
        Refine::Sniff(Sniff::Mcp),
    ),
];

fn glob(field: &'static str, pattern: &str) -> Result<Glob, DiscoverError> {
    GlobBuilder::new(pattern)
        .literal_separator(true)
        .build()
        .map_err(|source| DiscoverError::Glob {
            field,
            glob: pattern.to_owned(),
            source,
        })
}

fn glob_set(field: &'static str, patterns: &[String]) -> Result<GlobSet, DiscoverError> {
    let mut set = GlobSetBuilder::new();
    for pattern in patterns {
        set.add(glob(field, pattern)?);
    }
    set.build().map_err(|source| DiscoverError::Glob {
        field,
        glob: String::new(),
        source,
    })
}

/// Maps a rel path to its asset kind and load mode. Built once per scan.
#[derive(Debug)]
pub struct Classifier {
    /// Cheap file-name test implied by every pattern; `None` when a pattern's last segment
    /// is not a plain name or `*suffix`.
    names: Option<NameFilter>,
    set: GlobSet,
    /// Glob index in `set` → table entry. Lower index = higher priority.
    entries: Vec<Entry>,
    exclude: GlobSet,
    /// `scan.library`: built-in assets under these globs are OnDemand.
    library: GlobSet,
}

impl Classifier {
    /// Builds the table and library rows plus `scan.custom` (then `scan.include` as Custom),
    /// `scan.exclude` and `scan.library`.
    pub fn new(config: &Config) -> Result<Self, DiscoverError> {
        let mut set = GlobSetBuilder::new();
        let mut entries = Vec::new();
        let mut names = Some(NameFilter::default());
        let mut add = |field, pattern: &str, entry: Entry| -> Result<(), DiscoverError> {
            set.add(glob(field, pattern)?);
            entries.push(entry);
            names = names.take().and_then(|n| n.with(pattern));
            Ok(())
        };
        let library_rows = LIBRARY_TABLE
            .iter()
            .map(|row| (row, LoadMode::OnDemand, Origin::LibraryRow));
        let rows = TABLE
            .iter()
            .map(|row| (row, row.1.default_load(), Origin::Platform));
        for (&(patterns, kind, refine), load, origin) in rows.chain(library_rows) {
            let entry = Entry {
                kind,
                load,
                refine,
                origin,
            };
            for pattern in patterns {
                add("discovery table", pattern, entry)?;
            }
        }
        for decl in &config.scan.custom {
            let entry = Entry {
                kind: decl.kind,
                load: decl.load.unwrap_or(decl.kind.default_load()),
                refine: Refine::None,
                origin: Origin::Custom,
            };
            add("scan.custom", &decl.glob, entry)?;
        }
        for pattern in &config.scan.include {
            let entry = Entry {
                kind: AssetKind::Custom,
                load: AssetKind::Custom.default_load(),
                refine: Refine::None,
                origin: Origin::Custom,
            };
            add("scan.include", pattern, entry)?;
        }
        let set = set.build().map_err(|source| DiscoverError::Glob {
            field: "discovery table",
            glob: String::new(),
            source,
        })?;

        Ok(Self {
            names,
            set,
            entries,
            exclude: glob_set("scan.exclude", &config.scan.exclude)?,
            library: glob_set("scan.library", &config.scan.library)?,
        })
    }

    /// Classifies `rel_path` by name only. `scratch` is reused across calls to avoid allocation.
    /// Returns the entry and the content test it still needs, if any.
    fn lookup(&self, rel_path: &str, scratch: &mut Vec<usize>) -> Option<(Entry, Option<Sniff>)> {
        if let Some(names) = &self.names {
            let name = rel_path.rsplit('/').next().unwrap_or(rel_path);
            if !names.accepts(name) {
                return None;
            }
        }
        self.set
            .matches_candidate_into(&Candidate::new(rel_path), scratch);
        let entry = *self.entries.get(*scratch.iter().min()?)?;
        let (load, origin) = if entry.origin != Origin::Custom && self.in_library(rel_path) {
            (LoadMode::OnDemand, Origin::Library)
        } else {
            (resolve_load(entry, rel_path), entry.origin)
        };
        let sniff = match entry.refine {
            Refine::Sniff(sniff) => Some(sniff),
            _ => None,
        };
        Some((
            Entry {
                load,
                origin,
                ..entry
            },
            sniff,
        ))
    }

    /// Classifies `rel_path` by name only (no content sniffing).
    pub fn classify(&self, rel_path: &str) -> Option<(AssetKind, LoadMode)> {
        self.lookup(rel_path, &mut Vec::new())
            .map(|(e, _)| (e.kind, e.load))
    }

    /// Whether `rel_path` matches `scan.library`.
    fn in_library(&self, rel_path: &str) -> bool {
        !self.library.is_empty() && self.library.is_match(rel_path)
    }

    /// Whether `rel_path` matches `scan.exclude`.
    pub fn is_excluded(&self, rel_path: &str) -> bool {
        !self.exclude.is_empty() && self.exclude.is_match(rel_path)
    }
}

/// File names that can match the classifier's globs: a glob's last segment matches the
/// file name alone (`literal_separator`), so a file whose name is none of `names` and
/// ends with none of `suffixes` matches no glob. Skips the `GlobSet` for most files of a
/// repo (source code), a discovery hotspot (P7.1).
#[derive(Debug, Default)]
struct NameFilter {
    names: Vec<String>,
    suffixes: Vec<String>,
}

impl NameFilter {
    /// Adds `pattern`'s last segment, or gives up (`None`) if it is not a plain name or
    /// `*` followed by a plain suffix.
    fn with(mut self, pattern: &str) -> Option<Self> {
        let last = pattern.rsplit('/').next().unwrap_or(pattern);
        let plain = |s: &str| !s.contains(['*', '?', '[', ']', '{', '}', '\\']);
        match last.strip_prefix('*') {
            Some(suffix) if plain(suffix) => self.suffixes.push(suffix.to_owned()),
            None if plain(last) => self.names.push(last.to_owned()),
            _ => return None,
        }
        Some(self)
    }

    fn accepts(&self, name: &str) -> bool {
        self.suffixes.iter().any(|s| name.ends_with(s.as_str()))
            || self.names.iter().any(|n| n == name)
    }
}

/// Load mode from path alone; frontmatter-based refinements come later.
fn resolve_load(entry: Entry, rel_path: &str) -> LoadMode {
    match entry.refine {
        Refine::NestedConditional => {
            let parent = rel_path.rsplit_once('/').map_or("", |(dir, _)| dir);
            // `.claude/CLAUDE.md` is a root-level memory file in Claude Code.
            if parent.is_empty() || parent == ".claude" {
                LoadMode::Always
            } else {
                LoadMode::Conditional
            }
        }
        // Scoped: frontmatter promotion happens in `Asset::effective_load`.
        Refine::Scoped | Refine::None | Refine::Sniff(_) => entry.load,
    }
}

/// Reads at most 64 KiB of `path` and applies `sniff` (spec 02-assets → Content sniffing).
fn sniff_file(path: &Path, sniff: Sniff) -> io::Result<bool> {
    let mut buf = Vec::new();
    File::open(path)?.take(SNIFF_BYTES).read_to_end(&mut buf)?;
    let truncated = buf.len() as u64 >= SNIFF_BYTES;
    let bytes = buf.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&buf);
    Ok(match sniff {
        Sniff::Tools => sniff_tools(bytes, truncated),
        Sniff::Hooks => sniff_object(bytes, truncated, &["hooks"]),
        Sniff::Mcp => sniff_object(bytes, truncated, &["mcpServers", "servers"]),
    })
}

/// Tools: an object, or array of objects, with `name` plus `input_schema` | `parameters` |
/// `inputSchema`.
fn sniff_tools(bytes: &[u8], truncated: bool) -> bool {
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes) {
        return match &value {
            serde_json::Value::Array(items) => !items.is_empty() && items.iter().all(is_tool),
            other => is_tool(other),
        };
    }
    if !truncated {
        return false;
    }
    // Truncated: fall back to a key scan over the prefix.
    let has = |key: &[u8]| memchr::memmem::find(bytes, key).is_some();
    starts_with(bytes, b"{[")
        && has(b"\"name\"")
        && (has(b"\"input_schema\"") || has(b"\"parameters\"") || has(b"\"inputSchema\""))
}

/// Hooks / MCP configs: a JSONC object with one of `keys` holding an object. A truncated
/// prefix falls back to a key scan (starts with `{`, contains a quoted key).
fn sniff_object(bytes: &[u8], truncated: bool, keys: &[&str]) -> bool {
    if truncated {
        return starts_with(bytes, b"{")
            && keys.iter().any(|k| {
                let quoted = format!("\"{k}\"");
                memchr::memmem::find(bytes, quoted.as_bytes()).is_some()
            });
    }
    let Ok(source) = std::str::from_utf8(bytes) else {
        return false;
    };
    let doc = JsonDoc::parse(source);
    let Some(JsonValue::Object(members)) = doc.root.as_ref().map(|n| &n.value) else {
        return false;
    };
    members
        .iter()
        .any(|m| keys.contains(&m.key.as_str()) && matches!(m.value.value, JsonValue::Object(_)))
}

/// Whether the first non-whitespace byte is one of `firsts`.
fn starts_with(bytes: &[u8], firsts: &[u8]) -> bool {
    bytes
        .iter()
        .find(|b| !b.is_ascii_whitespace())
        .is_some_and(|b| firsts.contains(b))
}

fn is_tool(value: &serde_json::Value) -> bool {
    value.as_object().is_some_and(|o| {
        o.contains_key("name")
            && ["input_schema", "parameters", "inputSchema"]
                .iter()
                .any(|k| o.contains_key(*k))
    })
}

fn rel_of(root: &Path, path: &Path) -> Option<String> {
    let rel = match strip_root(root, path) {
        Some(rel) => rel,
        None => path.strip_prefix(root).ok()?.to_str()?,
    };
    Some(if rel.contains('\\') {
        rel.replace('\\', "/")
    } else {
        rel.to_owned()
    })
}

/// Fast path of [`rel_of`]: walked paths are `root` joined with the rest, so a byte prefix
/// check replaces `Path::strip_prefix`, which compares component by component (a
/// hotspot over 200k files, P7.1). `None` means "use the slow path".
fn strip_root<'a>(root: &Path, path: &'a Path) -> Option<&'a str> {
    let sep = |b: &u8| *b == b'/' || (cfg!(windows) && *b == b'\\');
    let root = root.as_os_str().as_encoded_bytes();
    if root.is_empty() {
        return None;
    }
    let rest = path.as_os_str().as_encoded_bytes().strip_prefix(root)?;
    let rest = match (root.last(), rest.split_first()) {
        (_, None) => rest,
        (_, Some((b, tail))) if sep(b) => tail,
        (Some(b), Some(_)) if sep(b) => rest,
        _ => return None,
    };
    if rest.first().is_some_and(sep) {
        return None;
    }
    std::str::from_utf8(rest).ok()
}

#[derive(Default)]
struct Collected {
    assets: Vec<DiscoveredFile>,
    /// Hashed once, after the walk: per-thread sets merged into a shared one cost two hashes
    /// and a rehash per path (P7.1).
    all_paths: Vec<String>,
    skipped: Vec<(PathBuf, String)>,
    misplaced: Vec<Misplaced>,
}

/// Per-thread buffer, merged into the shared one when the walker thread finishes.
struct Local<'a> {
    out: Collected,
    shared: &'a Mutex<Collected>,
}

impl Drop for Local<'_> {
    fn drop(&mut self) {
        let mut shared = self.shared.lock().unwrap_or_else(PoisonError::into_inner);
        let out = std::mem::take(&mut self.out);
        shared.assets.extend(out.assets);
        shared.all_paths.extend(out.all_paths);
        shared.skipped.extend(out.skipped);
        shared.misplaced.extend(out.misplaced);
    }
}

/// Walks `root` in parallel and classifies every file. Unreadable entries go to `skipped`.
pub fn discover(root: &Path, config: &Config) -> Result<Discovery, DiscoverError> {
    if !root.is_dir() {
        return Err(DiscoverError::NotADirectory(root.to_path_buf()));
    }
    let classifier = Classifier::new(config)?;
    let filter_root = root.to_path_buf();
    let exclude = classifier.exclude.clone();
    let walker = WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .git_global(false)
        .require_git(false)
        .parents(false)
        .add_custom_ignore_filename(PULSE_IGNORE)
        .follow_links(config.scan.follow_symlinks)
        .filter_entry(move |entry| keep_entry(entry, &filter_root, &exclude))
        .build_parallel();

    let shared = Mutex::new(Collected::default());
    walker.run(|| {
        let mut local = Local {
            out: Collected::default(),
            shared: &shared,
        };
        let mut scratch = Vec::new();
        let classifier = &classifier;
        Box::new(move |result| {
            match result {
                Ok(entry) => visit(&entry, root, classifier, &mut scratch, &mut local.out),
                Err(err) => {
                    let path = error_path(&err).unwrap_or_else(|| root.to_path_buf());
                    local.out.skipped.push((path, err.to_string()));
                }
            }
            WalkState::Continue
        })
    });

    let mut collected = shared.into_inner().unwrap_or_else(PoisonError::into_inner);
    collected
        .assets
        .sort_unstable_by(|a, b| a.rel_path.cmp(&b.rel_path));
    collected.skipped.sort_unstable();
    collected
        .misplaced
        .sort_unstable_by(|a, b| a.rel_path.cmp(&b.rel_path));
    let mut all_paths = HashSet::with_capacity(collected.all_paths.len());
    all_paths.extend(collected.all_paths);
    Ok(Discovery {
        assets: collected.assets,
        all_paths,
        skipped: collected.skipped,
        misplaced: collected.misplaced,
    })
}

fn keep_entry(entry: &DirEntry, root: &Path, exclude: &GlobSet) -> bool {
    if entry.depth() == 0 {
        return true;
    }
    let is_dir = entry.file_type().is_some_and(|t| t.is_dir());
    if is_dir
        && entry
            .file_name()
            .to_str()
            .is_some_and(|name| DEFAULT_EXCLUDES.contains(&name))
    {
        return false;
    }
    if exclude.is_empty() {
        return true;
    }
    rel_of(root, entry.path()).is_none_or(|rel| !exclude.is_match(rel.as_str()))
}

fn visit(
    entry: &DirEntry,
    root: &Path,
    classifier: &Classifier,
    scratch: &mut Vec<usize>,
    out: &mut Collected,
) {
    let Some(file_type) = entry.file_type() else {
        return;
    };
    // Unfollowed symlinks (e.g. `CLAUDE.md -> AGENTS.md`) exist for AI-STR-003 but are not
    // analyzed twice: their target is walked on its own.
    if file_type.is_symlink() {
        if let Some(rel_path) = rel_of(root, entry.path()) {
            out.all_paths.push(rel_path);
        }
        return;
    }
    if !file_type.is_file() {
        return;
    }
    let path = entry.path();
    let Some(rel_path) = rel_of(root, path) else {
        out.skipped
            .push((path.to_path_buf(), "path is not valid UTF-8".to_owned()));
        return;
    };
    let mut is_asset = false;
    if let Some((entry, sniff)) = classifier.lookup(&rel_path, scratch) {
        is_asset = sniff.is_none_or(|sniff| match sniff_file(path, sniff) {
            Ok(matches) => matches,
            Err(err) => {
                out.skipped.push((path.to_path_buf(), err.to_string()));
                false
            }
        });
        if is_asset {
            out.assets.push(DiscoveredFile {
                path: Arc::from(path),
                rel_path: rel_path.clone(),
                kind: entry.kind,
                load: entry.load,
                origin: entry.origin,
            });
        }
    }
    // AI-STR-008: a name check on files that are not assets; content is never read.
    if !is_asset
        && let Some(near_miss) = near_miss(&rel_path)
        && !classifier.in_library(&rel_path)
    {
        out.misplaced.push(Misplaced {
            rel_path: rel_path.clone(),
            near_miss,
        });
    }
    out.all_paths.push(rel_path);
}

fn error_path(err: &ignore::Error) -> Option<PathBuf> {
    match err {
        ignore::Error::WithPath { path, .. } => Some(path.clone()),
        ignore::Error::WithDepth { err, .. } | ignore::Error::WithLineNumber { err, .. } => {
            error_path(err)
        }
        ignore::Error::Loop { child, .. } => Some(child.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CustomAssetDecl;

    fn classify(rel: &str) -> Option<(AssetKind, LoadMode)> {
        Classifier::new(&Config::default())
            .ok()
            .and_then(|c| c.classify(rel))
    }

    #[test]
    fn classify_root_and_nested_instructions() {
        assert_eq!(
            classify("CLAUDE.md"),
            Some((AssetKind::Instructions, LoadMode::Always))
        );
        assert_eq!(
            classify(".claude/CLAUDE.md"),
            Some((AssetKind::Instructions, LoadMode::Always))
        );
        assert_eq!(
            classify("pkg/AGENTS.md"),
            Some((AssetKind::Instructions, LoadMode::Conditional))
        );
        assert_eq!(
            classify("pkg/GEMINI.md"),
            Some((AssetKind::Instructions, LoadMode::Always))
        );
    }

    #[test]
    fn classify_clinerules_file_and_dir() {
        assert_eq!(
            classify(".clinerules"),
            Some((AssetKind::Instructions, LoadMode::Always))
        );
        assert_eq!(
            classify(".clinerules/style.md"),
            Some((AssetKind::ScopedInstructions, LoadMode::Conditional))
        );
    }

    #[test]
    fn classify_first_match_wins() {
        // Row 4 (Skill) beats row 5 (Agent) for a SKILL.md under .claude/agents.
        assert_eq!(
            classify(".claude/agents/SKILL.md"),
            Some((AssetKind::Skill, LoadMode::Always))
        );
        // `*` does not cross `/`: tools/a/b.json is not a tools/*.json candidate.
        assert_eq!(classify("tools/a/b.json"), None);
        assert_eq!(classify("README.md"), None);
    }

    #[test]
    fn classify_library_rows_after_platform_rows() {
        use AssetKind::*;
        use LoadMode::*;
        for (rel, want) in [
            ("agents/a.agent.md", Some((Agent, OnDemand))),
            ("x/b.chatmode.md", Some((Agent, OnDemand))),
            ("p/c.prompt.md", Some((Prompt, OnDemand))),
            ("i/d.instructions.md", Some((ScopedInstructions, OnDemand))),
            ("hooks/x/hooks.json", Some((Hooks, OnDemand))),
            ("mcp.json", Some((McpConfig, OnDemand))),
            // Platform rows still win.
            (".github/agents/a.agent.md", Some((Agent, Always))),
            (".github/prompts/c.prompt.md", Some((Prompt, OnDemand))),
            (
                ".github/instructions/d.instructions.md",
                Some((ScopedInstructions, Conditional)),
            ),
            ("plugin/hooks/hooks.json", Some((Hooks, Always))),
            (".vscode/mcp.json", Some((McpConfig, Always))),
            (".kiro/settings/mcp.json", Some((McpConfig, Always))),
            // Examples are not loaded by Kiro.
            (".kiro/settings/mcp.json.example", None),
            ("agent.md", None),
        ] {
            assert_eq!(classify(rel), want, "{rel}");
        }
    }

    #[test]
    fn scan_library_applies_to_builtin_rows_only() {
        let mut config = Config::default();
        config.scan.library = vec!["lib/**".to_owned()];
        config.scan.custom.push(CustomAssetDecl {
            glob: "lib/*.txt".to_owned(),
            kind: AssetKind::Instructions,
            load: Some(LoadMode::Always),
        });
        let c = Classifier::new(&config).expect("classifier");
        assert_eq!(
            c.classify("lib/x/SKILL.md"),
            Some((AssetKind::Skill, LoadMode::OnDemand))
        );
        assert_eq!(
            c.classify("lib/CLAUDE.md"),
            Some((AssetKind::Instructions, LoadMode::OnDemand))
        );
        assert_eq!(
            c.classify("lib/a.txt"),
            Some((AssetKind::Instructions, LoadMode::Always))
        );
        assert_eq!(
            c.classify("skills/x/SKILL.md"),
            Some((AssetKind::Skill, LoadMode::Always))
        );
    }

    #[test]
    fn lookup_records_origin() {
        let mut config = Config::default();
        config.scan.library = vec!["lib/**".to_owned()];
        config.scan.include = vec!["lib/*.txt".to_owned()];
        let c = Classifier::new(&config).expect("classifier");
        let origin = |rel: &str| c.lookup(rel, &mut Vec::new()).map(|(e, _)| e.origin);
        assert_eq!(origin(".github/agents/a.agent.md"), Some(Origin::Platform));
        assert_eq!(origin("agents/a.agent.md"), Some(Origin::LibraryRow));
        assert_eq!(origin("lib/a.agent.md"), Some(Origin::Library));
        assert_eq!(origin("lib/x/SKILL.md"), Some(Origin::Library));
        assert_eq!(origin("lib/a.txt"), Some(Origin::Custom));
    }

    #[test]
    fn walk_records_misplaced_outside_library() {
        let Ok(dir) = tempfile::tempdir() else { return };
        for rel in [
            "claude.md",
            ".claude/agent/a.md",
            "lib/skill.md",
            "CLAUDE.md",
        ] {
            let path = dir.path().join(rel);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            std::fs::write(path, "x").expect("write");
        }
        let mut config = Config::default();
        config.scan.library = vec!["lib/**".to_owned()];
        let found = discover(dir.path(), &config).expect("discover");
        let rels: Vec<&str> = found.misplaced.iter().map(|m| &*m.rel_path).collect();
        assert_eq!(rels, [".claude/agent/a.md", "claude.md"]);
    }

    #[test]
    fn classify_custom_and_include() {
        let mut config = Config::default();
        config.scan.custom.push(CustomAssetDecl {
            glob: "prompts/**/*.txt".to_owned(),
            kind: AssetKind::Prompt,
            load: None,
        });
        config.scan.include.push("docs/ai/*.md".to_owned());
        let c = Classifier::new(&config).ok();
        let c = c.as_ref();
        assert_eq!(
            c.and_then(|c| c.classify("prompts/a/x.txt")),
            Some((AssetKind::Prompt, LoadMode::OnDemand))
        );
        assert_eq!(
            c.and_then(|c| c.classify("docs/ai/x.md")),
            Some((AssetKind::Custom, LoadMode::OnDemand))
        );
    }

    #[test]
    fn name_filter_skips_only_impossible_names() {
        let c = Classifier::new(&Config::default()).expect("classifier");
        let names = c.names.as_ref().expect("built-in table is filterable");
        for name in [
            "CLAUDE.md",
            "x.mdc",
            ".cursorrules",
            "a.prompty",
            "mcp.json",
            "SKILL.md",
        ] {
            assert!(names.accepts(name), "{name}");
        }
        for name in ["main.rs", "CLAUDE.txt", "Makefile", "md"] {
            assert!(!names.accepts(name), "{name}");
        }
        // A complex last segment disables the filter rather than missing assets.
        assert!(
            NameFilter::default()
                .with("**/prompts/*.{md,txt}")
                .is_none()
        );
        assert!(NameFilter::default().with("**/AI-?.md").is_none());
        let mut cfg = Config::default();
        cfg.scan.include = vec!["docs/ai/*.txt".to_owned()];
        let c = Classifier::new(&cfg).expect("classifier");
        assert_eq!(
            c.classify("docs/ai/x.txt").map(|(k, _)| k),
            Some(AssetKind::Custom)
        );
        assert_eq!(c.classify("src/main.rs"), None);
    }

    #[test]
    fn rel_of_fast_and_slow_paths() {
        let rel = |root: &str, path: &str| rel_of(Path::new(root), Path::new(path));
        assert_eq!(
            rel("repo", "repo/a/CLAUDE.md").as_deref(),
            Some("a/CLAUDE.md")
        );
        assert_eq!(rel("repo/", "repo/a.md").as_deref(), Some("a.md"));
        assert_eq!(rel(".", "./x/y.md").as_deref(), Some("x/y.md"));
        assert_eq!(rel("repo", "repo").as_deref(), Some(""));
        // Not a component prefix: the fast path declines, the slow path rejects.
        assert_eq!(rel("rep", "repo/a.md"), None);
        assert_eq!(strip_root(Path::new("rep"), Path::new("repo/a.md")), None);
        // Same components, different bytes: the slow path still matches.
        assert_eq!(rel("repo", "repo//a.md").as_deref(), Some("a.md"));
    }

    #[test]
    fn classifier_rejects_bad_glob() {
        let mut config = Config::default();
        config.scan.exclude.push("a/[".to_owned());
        assert!(matches!(
            Classifier::new(&config),
            Err(DiscoverError::Glob {
                field: "scan.exclude",
                ..
            })
        ));
    }

    #[test]
    fn sniff_tools_shapes() {
        let dir = tempfile::tempdir().ok();
        let Some(dir) = dir else { return };
        let check = |body: &str| {
            let p = dir.path().join("t.json");
            std::fs::write(&p, body).ok();
            sniff_file(&p, Sniff::Tools).ok()
        };
        assert_eq!(check(r#"[{"name":"a","input_schema":{}}]"#), Some(true));
        assert_eq!(check(r#"{"name":"a","parameters":{}}"#), Some(true));
        assert_eq!(check(r#"{"name":"a"}"#), Some(false));
        assert_eq!(check("[]"), Some(false));
        assert_eq!(check("not json"), Some(false));
        let big = format!(
            r#"[{{"name":"a","inputSchema":{{"description":"{}"}}}}]"#,
            "x".repeat(70_000)
        );
        assert_eq!(check(&big), Some(true));
    }

    #[test]
    fn sniff_hooks_and_mcp_shapes() {
        let hooks = |body: &str| sniff_object(body.as_bytes(), false, &["hooks"]);
        let mcp = |body: &str| sniff_object(body.as_bytes(), false, &["mcpServers", "servers"]);
        assert!(hooks("{ // jsonc\n \"hooks\": {}, }"));
        assert!(!hooks(r#"{"hooks": []}"#));
        assert!(!hooks(r#"[{"hooks": {}}]"#));
        assert!(!hooks("{"));
        assert!(mcp(r#"{"mcpServers": {"a": {}}}"#));
        assert!(mcp(r#"{"servers": {}, "inputs": []}"#));
        assert!(!mcp(r#"{"name": "mcp", "servers": 3}"#));
        assert!(!mcp(""));
        let big = format!(r#"{{"hooks": {{"x": "{}"#, "y".repeat(70_000));
        assert!(sniff_object(big.as_bytes(), true, &["hooks"]));
        assert!(!sniff_object(big.as_bytes(), true, &["servers"]));
    }
}
