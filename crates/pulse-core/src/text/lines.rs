//! Byte offset → (line, column) mapping.

/// Start offsets of every line in a source, for fast byte → (line, col) lookups.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineIndex {
    /// Byte offset of the first byte of each line. Always starts with `0`.
    starts: Vec<u32>,
}

impl LineIndex {
    /// Indexes `source` by scanning for `\n` (so `\r\n` line endings work unchanged).
    pub fn new(source: &str) -> Self {
        let mut starts = Vec::with_capacity(source.len() / 32 + 1);
        starts.push(0);
        starts.extend(memchr::memchr_iter(b'\n', source.as_bytes()).map(|i| to_u32(i + 1)));
        Self { starts }
    }

    /// Number of lines (an empty source, or one ending in `\n`, still has a last empty line).
    pub fn line_count(&self) -> usize {
        self.starts.len()
    }

    /// 1-based `(line, col)` of byte offset `byte` in `source`; `col` counts chars, not bytes.
    ///
    /// `source` must be the string this index was built from. Offsets past the end clamp
    /// to the end, and offsets inside a multibyte char snap back to its start.
    pub fn location(&self, source: &str, byte: u32) -> (u32, u32) {
        let mut byte = (byte as usize).min(source.len());
        while !source.is_char_boundary(byte) {
            byte -= 1;
        }
        // Index of the last line start <= byte; `starts[0] == 0` guarantees a match.
        let line = self.starts.partition_point(|&s| s as usize <= byte) - 1;
        let start = self.starts[line] as usize;
        let col = source.get(start..byte).map_or(0, |s| s.chars().count());
        (to_u32(line + 1), to_u32(col + 1))
    }
}

fn to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loc(src: &str, byte: u32) -> (u32, u32) {
        LineIndex::new(src).location(src, byte)
    }

    #[test]
    fn empty_source() {
        let idx = LineIndex::new("");
        assert_eq!(idx.line_count(), 1);
        assert_eq!(idx.location("", 0), (1, 1));
        assert_eq!(idx.location("", 10), (1, 1));
    }

    #[test]
    fn lf_lines() {
        let src = "ab\ncd\n";
        assert_eq!(loc(src, 0), (1, 1));
        assert_eq!(loc(src, 2), (1, 3)); // the '\n' itself
        assert_eq!(loc(src, 3), (2, 1));
        assert_eq!(loc(src, 4), (2, 2));
    }

    #[test]
    fn crlf_lines() {
        let src = "ab\r\ncd\r\nef";
        assert_eq!(LineIndex::new(src).line_count(), 3);
        assert_eq!(loc(src, 2), (1, 3)); // '\r'
        assert_eq!(loc(src, 4), (2, 1));
        assert_eq!(loc(src, 8), (3, 1));
        assert_eq!(loc(src, 9), (3, 2));
    }

    #[test]
    fn multibyte_chars_count_as_one_column() {
        let src = "é😀x\nñy";
        // 'é' = 2 bytes, '😀' = 4 bytes.
        assert_eq!(loc(src, 2), (1, 2));
        assert_eq!(loc(src, 6), (1, 3));
        assert_eq!(loc(src, 10), (2, 2));
        // Inside the emoji snaps back to its start.
        assert_eq!(loc(src, 4), (1, 2));
    }

    #[test]
    fn offset_at_and_past_eof() {
        let src = "a\nbc";
        assert_eq!(loc(src, 4), (2, 3));
        assert_eq!(loc(src, 99), (2, 3));
        let trailing = "a\n";
        assert_eq!(loc(trailing, 2), (2, 1));
    }
}
