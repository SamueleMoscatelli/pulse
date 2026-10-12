//! A discovered AI asset: its source text plus lazily parsed views.

use std::path::Path;
use std::sync::Arc;

use crate::model::{AssetKind, LoadMode, Location, Span};
use crate::text::LineIndex;

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
    // --- Parsed views (P1.x) ---
    // `OnceLock` caches go here: `markdown()`, `json()`, `normalized()`, `tokens()`.
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
}
