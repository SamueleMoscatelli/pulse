//! File references (spec 03-rules → AI-STR-003): path resolution and the `@import` graph.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::parse::FileRef;
use crate::{Asset, AssetKind, KindSet, LoadMode};

/// Import hops followed from a file (Claude Code's limit).
pub const MAX_DEPTH: usize = 5;

/// Resolves `target`, as written in the file `from` (both scan-root-relative), to a
/// normalized scan-root-relative path. A leading `/` means the scan root. `None` when the
/// target can't be checked: empty, home-relative (`~`) or escaping the scan root.
pub fn resolve(from: &str, target: &str) -> Option<String> {
    resolve_in(from.rsplit_once('/').map_or("", |(dir, _)| dir), target)
}

/// Like [`resolve`], relative to the scan-root-relative directory `dir` (`""` is the root).
pub fn resolve_in(dir: &str, target: &str) -> Option<String> {
    if target.is_empty() || target.starts_with('~') {
        return None;
    }
    let (base, rest) = match target.strip_prefix('/') {
        Some(rest) => ("", rest),
        None => (dir, target),
    };
    let mut parts: Vec<&str> = Vec::new();
    for part in base.split('/').chain(rest.split('/')) {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            p => parts.push(p),
        }
    }
    if parts.is_empty() {
        return None;
    }
    Some(parts.join("/"))
}

/// Whether an `@` token names a file: it has a `/` or a `.` (`@docs/x.md`, `@README.md`),
/// unlike a mention (`@alice`, `@copilot`).
pub fn is_file_import(path: &str) -> bool {
    path.contains(['/', '.'])
}

/// The directory Kiro `#[[file:…]]` references in `rel` resolve from: the workspace root
/// (the folder holding `.kiro/`) for steering files, the containing folder for `AGENTS.md`.
/// `None` for files where Kiro doesn't expand references.
pub fn kiro_ref_base(rel: &str) -> Option<&str> {
    const STEERING: &str = ".kiro/steering/";
    if let Some(i) = rel
        .match_indices(STEERING)
        .map(|(i, _)| i)
        .find(|&i| i == 0 || rel.as_bytes()[i - 1] == b'/')
    {
        return Some(rel[..i].trim_end_matches('/'));
    }
    match rel.rsplit_once('/') {
        Some((dir, "AGENTS.md")) => Some(dir),
        None if rel == "AGENTS.md" => Some(""),
        _ => None,
    }
}

/// Resolves the Kiro `#[[file:…]]` / `#[[folder:…]]` references of `asset`, in source
/// order, paired with their resolved paths; empty where Kiro doesn't expand them.
pub fn kiro_refs(asset: &Asset) -> impl Iterator<Item = (&FileRef, String)> {
    let base = kiro_ref_base(&asset.rel_path);
    let refs = if base.is_some() && KindSet::MARKDOWN.contains(asset.kind) {
        asset.markdown().file_refs.as_slice()
    } else {
        &[]
    };
    refs.iter()
        .filter_map(move |r| Some((r, resolve_in(base?, &r.path)?)))
}

/// Resolved `@imports` (and Kiro `#[[file:…]]` references) of every Markdown asset and, transitively up to [`MAX_DEPTH`] hops,
/// of the walked files they import. Imported files that aren't assets are read once
/// (bounded by `scan.max_file_bytes`) and kept for their tokens.
#[derive(Debug, Default)]
pub struct ImportGraph {
    /// Resolved targets of existing files, by importing file; only files with imports.
    edges: HashMap<String, Vec<String>>,
    /// Imported walked files that aren't assets, keyed by relative path.
    files: HashMap<String, Asset>,
    /// `(from, target)` edges that only bring in a line range (Kiro `#[[file:p:3-7]]`).
    ranges: HashMap<(String, String), (u32, u32)>,
}

impl ImportGraph {
    /// Builds the graph. `files` is every walked path; targets outside it are not followed.
    pub fn build(
        assets: &[Asset],
        files: &HashSet<String>,
        root: &Path,
        max_bytes: u64,
        calibration: f64,
    ) -> Self {
        let mut graph = Self::default();
        let known: HashSet<&str> = assets.iter().map(|a| a.rel_path.as_str()).collect();
        let mut queue = VecDeque::new();
        for a in assets {
            if KindSet::MARKDOWN.contains(a.kind) && a.skipped.is_none() && !a.is_json_agent() {
                graph.add_edges(a, files, &mut queue, 1);
            }
        }
        while let Some((rel, depth)) = queue.pop_front() {
            if known.contains(rel.as_str()) || graph.files.contains_key(&rel) {
                continue;
            }
            let file = read(root, &rel, max_bytes, calibration);
            if depth < MAX_DEPTH && file.kind == AssetKind::Instructions {
                graph.add_edges(&file, files, &mut queue, depth + 1);
            }
            graph.files.insert(rel, file);
        }
        graph
    }

    fn add_edges(
        &mut self,
        file: &Asset,
        files: &HashSet<String>,
        queue: &mut VecDeque<(String, usize)>,
        depth: usize,
    ) {
        let mut targets: Vec<String> = file
            .markdown()
            .imports
            .iter()
            .filter(|i| is_file_import(&i.path))
            .filter_map(|i| resolve(&file.rel_path, &i.path))
            .filter(|t| files.contains(t))
            .collect();
        let mut whole: HashSet<String> = targets.iter().cloned().collect();
        for (r, target) in kiro_refs(file) {
            if r.folder || !files.contains(&target) {
                continue;
            }
            let key = (file.rel_path.clone(), target.clone());
            match r.lines {
                Some(lines) if !whole.contains(&target) => {
                    // The first range wins; a whole-file reference anywhere overrides it.
                    self.ranges.entry(key).or_insert(lines);
                }
                Some(_) => {}
                None => {
                    self.ranges.remove(&key);
                    whole.insert(target.clone());
                }
            }
            if !targets.contains(&target) {
                targets.push(target);
            }
        }
        if targets.is_empty() {
            return;
        }
        queue.extend(targets.iter().map(|t| (t.clone(), depth)));
        self.edges.insert(file.rel_path.clone(), targets);
    }

    /// Resolved, existing import targets of `rel`.
    pub fn imports(&self, rel: &str) -> &[String] {
        self.edges.get(rel).map_or(&[], Vec::as_slice)
    }

    /// The 1-based inclusive line range that `from` brings in from `target`, when it
    /// references only those lines.
    pub fn range(&self, from: &str, target: &str) -> Option<(u32, u32)> {
        self.ranges
            .get(&(from.to_owned(), target.to_owned()))
            .copied()
    }

    /// An imported walked file that isn't an asset.
    pub fn file(&self, rel: &str) -> Option<&Asset> {
        self.files.get(rel)
    }

    /// Files reachable from `start` within [`MAX_DEPTH`] hops (excluding `start`), each with
    /// the direct import of `start` it was first reached through. Sorted by path.
    pub fn reachable(&self, start: &str) -> BTreeMap<&str, &str> {
        let mut seen: BTreeMap<&str, &str> = BTreeMap::new();
        let mut frontier: Vec<(&str, &str)> = self
            .imports(start)
            .iter()
            .map(|t| (t.as_str(), t.as_str()))
            .collect();
        for _ in 0..MAX_DEPTH {
            let mut next = Vec::new();
            for (rel, via) in frontier {
                if rel == start || seen.contains_key(rel) {
                    continue;
                }
                seen.insert(rel, via);
                next.extend(self.imports(rel).iter().map(|t| (t.as_str(), via)));
            }
            frontier = next;
        }
        seen
    }

    /// Whether importing `target` from `from` closes a cycle of at most [`MAX_DEPTH`] hops.
    pub fn closes_cycle(&self, from: &str, target: &str) -> bool {
        if from == target {
            return true;
        }
        let mut seen = HashSet::new();
        let mut frontier = vec![target];
        for _ in 1..MAX_DEPTH {
            let mut next = Vec::new();
            for rel in frontier {
                for t in self.imports(rel) {
                    if t == from {
                        return true;
                    }
                    if seen.insert(t.as_str()) {
                        next.push(t.as_str());
                    }
                }
            }
            frontier = next;
        }
        false
    }
}

/// Reads an imported file; Markdown becomes an Instructions asset (so its imports are
/// followed), anything else Custom. Unreadable or oversized files are skipped (0 tokens).
fn read(root: &Path, rel: &str, max_bytes: u64, calibration: f64) -> Asset {
    let path = root.join(rel);
    let kind = if rel.ends_with(".md") || rel.ends_with(".mdc") {
        AssetKind::Instructions
    } else {
        AssetKind::Custom
    };
    let mut bytes = Vec::new();
    let res =
        File::open(&path).and_then(|f| f.take(max_bytes.saturating_add(1)).read_to_end(&mut bytes));
    match res {
        Ok(n) if n as u64 <= max_bytes => {
            Asset::from_bytes(path, rel, kind, LoadMode::Always, bytes)
                .with_calibration(calibration)
        }
        Ok(_) => Asset::skipped(
            path,
            rel,
            kind,
            LoadMode::Always,
            "over scan.max_file_bytes",
        ),
        Err(e) => Asset::skipped(
            path,
            rel,
            kind,
            LoadMode::Always,
            format!("cannot read: {e}"),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_normalizes_relative_and_root_paths() {
        assert_eq!(
            resolve("CLAUDE.md", "docs/a.md").as_deref(),
            Some("docs/a.md")
        );
        assert_eq!(resolve("x/CLAUDE.md", "./a.md").as_deref(), Some("x/a.md"));
        assert_eq!(resolve("x/y/f.md", "../a.md").as_deref(), Some("x/a.md"));
        assert_eq!(
            resolve("x/f.md", "/docs/a.md").as_deref(),
            Some("docs/a.md")
        );
        assert_eq!(resolve("f.md", "../out.md"), None);
        assert_eq!(resolve("f.md", "~/.claude/x.md"), None);
        assert_eq!(resolve("f.md", ""), None);
    }

    #[test]
    fn file_imports_need_a_slash_or_dot() {
        assert!(is_file_import("docs/a"));
        assert!(is_file_import("README.md"));
        assert!(!is_file_import("alice"));
    }

    fn md(rel: &str, src: &str) -> Asset {
        Asset::new(
            Path::new(rel),
            rel,
            AssetKind::Instructions,
            LoadMode::Always,
            src.to_owned(),
        )
    }

    #[test]
    fn graph_reads_imported_files_and_follows_them() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("docs")).unwrap();
        std::fs::write(dir.path().join("docs/a.md"), "See @b.md for more words.\n").unwrap();
        std::fs::write(dir.path().join("docs/b.md"), "Loop @../CLAUDE.md\n").unwrap();
        let assets = [md("CLAUDE.md", "Read @docs/a.md and @docs/missing.md.\n")];
        let files: HashSet<String> = ["CLAUDE.md", "docs/a.md", "docs/b.md"]
            .map(str::to_owned)
            .into();
        let g = ImportGraph::build(&assets, &files, dir.path(), 1 << 20, 1.0);
        assert_eq!(g.imports("CLAUDE.md"), ["docs/a.md"]);
        assert!(g.file("docs/a.md").is_some_and(|f| f.tokens() > 0));
        assert!(g.file("CLAUDE.md").is_none());
        let r = g.reachable("CLAUDE.md");
        assert_eq!(
            r.keys().copied().collect::<Vec<_>>(),
            ["docs/a.md", "docs/b.md"]
        );
        assert_eq!(r["docs/b.md"], "docs/a.md");
        assert!(g.closes_cycle("CLAUDE.md", "docs/a.md"));
        assert!(!g.closes_cycle("docs/a.md", "docs/missing.md"));
    }

    #[test]
    fn kiro_refs_resolve_from_workspace_root_or_agents_dir() {
        assert_eq!(kiro_ref_base(".kiro/steering/a.md"), Some(""));
        assert_eq!(kiro_ref_base("pkg/.kiro/steering/x/a.md"), Some("pkg"));
        assert_eq!(kiro_ref_base("AGENTS.md"), Some(""));
        assert_eq!(kiro_ref_base("pkg/AGENTS.md"), Some("pkg"));
        for rel in ["CLAUDE.md", "my.kiro/steering/a.md", "docs/x.md"] {
            assert_eq!(kiro_ref_base(rel), None, "{rel}");
        }
    }

    #[test]
    fn graph_follows_kiro_refs_and_keeps_line_ranges() {
        let steering = Asset::new(
            Path::new(".kiro/steering/a.md"),
            ".kiro/steering/a.md",
            AssetKind::ScopedInstructions,
            LoadMode::Conditional,
            "#[[file:docs/a.md:2-3]] #[[file:docs/b.md:1]] #[[file:docs/b.md]] \
             #[[folder:docs]] #[[file:docs/missing.md]]\n"
                .to_owned(),
        );
        let files: HashSet<String> = [".kiro/steering/a.md", "docs/a.md", "docs/b.md"]
            .map(str::to_owned)
            .into();
        let g = ImportGraph::build(&[steering], &files, Path::new("."), 1 << 20, 1.0);
        let from = ".kiro/steering/a.md";
        assert_eq!(g.imports(from), ["docs/a.md", "docs/b.md"]);
        assert_eq!(g.range(from, "docs/a.md"), Some((2, 3)));
        assert_eq!(g.range(from, "docs/b.md"), None);
    }

    #[test]
    fn graph_skips_oversized_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("big.md"), "x".repeat(100)).unwrap();
        let assets = [md("CLAUDE.md", "@big.md\n")];
        let files: HashSet<String> = ["CLAUDE.md", "big.md"].map(str::to_owned).into();
        let g = ImportGraph::build(&assets, &files, dir.path(), 10, 1.0);
        assert!(
            g.file("big.md")
                .is_some_and(|f| f.skipped.is_some() && f.tokens() == 0)
        );
    }

    #[test]
    fn cycles_beyond_max_depth_are_not_reported() {
        let names = ["f0.md", "f1.md", "f2.md", "f3.md", "f4.md", "f5.md"];
        let assets: Vec<Asset> = (0..names.len())
            .map(|i| md(names[i], &format!("@{}\n", names[(i + 1) % names.len()])))
            .collect();
        let files: HashSet<String> = names.map(str::to_owned).into();
        let g = ImportGraph::build(&assets, &files, Path::new("."), 1 << 20, 1.0);
        assert!(!g.closes_cycle("f0.md", "f1.md"));
        let five: Vec<Asset> = assets
            .into_iter()
            .take(4)
            .chain([md("f4.md", "@f0.md\n")])
            .collect();
        let g = ImportGraph::build(&five, &files, Path::new("."), 1 << 20, 1.0);
        assert!(g.closes_cycle("f0.md", "f1.md"));
    }
}
