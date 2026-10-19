//! Markdown view: YAML frontmatter plus the body structure rules need (spec 02-assets →
//! Per-kind parsed views). All spans are byte offsets into the whole source.

use std::collections::BTreeMap;
use std::ops::Range;

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use serde_yaml_ng::{Mapping, Value};

use crate::model::Span;
use crate::text::LineIndex;

/// Parsed structure of a Markdown asset.
#[derive(Debug, Clone, Default)]
pub struct MarkdownView {
    pub frontmatter: Option<Frontmatter>,
    /// Everything after the frontmatter (the whole file when there is none).
    pub body: Span,
    pub headings: Vec<Heading>,
    pub code_blocks: Vec<CodeBlock>,
    /// `<!-- … -->`, delimiters included; an unclosed comment runs to the end of its HTML.
    pub html_comments: Vec<Span>,
    /// Raw HTML outside code: whole HTML blocks and single inline tags, in source order.
    pub html: Vec<Span>,
    pub links: Vec<Link>,
    pub imports: Vec<Import>,
    pub paragraphs: Vec<Span>,
    pub tables: Vec<Table>,
}

/// A `---`-delimited YAML block at the very start of the file.
#[derive(Debug, Clone)]
pub struct Frontmatter {
    /// The whole block, delimiter lines included.
    pub span: Span,
    /// The YAML text between the delimiters.
    pub body_span: Span,
    pub value: Result<Mapping, YamlError>,
    /// 1-based file line of each top-level key (first occurrence), from a line scan.
    pub key_lines: BTreeMap<String, u32>,
}

impl Frontmatter {
    /// The value of top-level `key`, if the YAML parsed.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.value.as_ref().ok()?.get(key)
    }
}

/// A frontmatter that is not a valid YAML mapping.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("line {line}: {msg}")]
pub struct YamlError {
    /// 1-based file line.
    pub line: u32,
    pub msg: String,
}

/// An ATX or setext heading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    /// 1–6.
    pub level: u8,
    /// Plain text content (inline code included, markup dropped).
    pub text: String,
    pub span: Span,
}

/// A fenced or indented code block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeBlock {
    /// First word of the fence info string; `None` when empty or indented.
    pub lang: Option<String>,
    /// The whole block, fences included.
    pub span: Span,
    /// 1-based inclusive `(first, last)` file lines.
    pub lines: (u32, u32),
}

/// A GFM table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    pub span: Span,
    /// Body rows (the header row excluded).
    pub rows: u32,
}

/// A link or image (inline, reference or autolink).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub dest: String,
    pub is_image: bool,
    pub span: Span,
}

/// A CLAUDE.md-style `@path` import.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Import {
    /// The path without the leading `@`.
    pub path: String,
    /// `@` included.
    pub span: Span,
}

impl MarkdownView {
    /// Parses `source`; `lines` must index the same string. Never fails: bad YAML is kept as `Err`.
    pub fn parse(source: &str, lines: &LineIndex) -> Self {
        let (frontmatter, body_start) = match split_frontmatter(source) {
            Some((span, body_span)) => (
                Some(parse_frontmatter(source, span, body_span, lines)),
                span.end as usize,
            ),
            None => (None, 0),
        };
        let mut view = Self {
            frontmatter,
            body: Span::new(to_u32(body_start), to_u32(source.len())),
            ..Self::default()
        };
        let body = source.get(body_start..).unwrap_or_default();
        // Inline code, code blocks, raw HTML and links: no `@imports` inside them
        // (a link's `@` is a URL part, e.g. `https://medium.com/@user`).
        let mut code: Vec<Span> = Vec::new();
        let mut heading: Option<Heading> = None;
        let sp =
            |r: Range<usize>| Span::new(to_u32(body_start + r.start), to_u32(body_start + r.end));
        let opts = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
        for (event, range) in Parser::new_ext(body, opts).into_offset_iter() {
            match event {
                Event::Start(Tag::Heading { level, .. }) => {
                    heading = Some(Heading {
                        level: level as u8,
                        text: String::new(),
                        span: sp(range),
                    });
                }
                Event::End(TagEnd::Heading(_)) => view.headings.extend(heading.take()),
                Event::Start(Tag::CodeBlock(kind)) => {
                    let lang = match kind {
                        CodeBlockKind::Fenced(info) => {
                            info.split_whitespace().next().map(str::to_owned)
                        }
                        CodeBlockKind::Indented => None,
                    };
                    let span = sp(range);
                    let last = span.end.saturating_sub(1).max(span.start);
                    view.code_blocks.push(CodeBlock {
                        lang,
                        span,
                        lines: (lines.line_of(span.start), lines.line_of(last)),
                    });
                    code.push(span);
                }
                Event::Start(tag @ (Tag::Link { .. } | Tag::Image { .. })) => {
                    let is_image = matches!(tag, Tag::Image { .. });
                    let (Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. }) = tag else {
                        continue;
                    };
                    let span = sp(range);
                    view.links.push(Link {
                        dest: dest_url.into_string(),
                        is_image,
                        span,
                    });
                    code.push(span);
                }
                Event::Start(Tag::Paragraph) => view.paragraphs.push(sp(range)),
                Event::Start(Tag::Table(_)) => view.tables.push(Table {
                    span: sp(range),
                    rows: 0,
                }),
                Event::Start(Tag::TableRow) => {
                    if let Some(t) = view.tables.last_mut() {
                        t.rows += 1;
                    }
                }
                Event::Start(Tag::HtmlBlock) | Event::InlineHtml(_) => {
                    let span = sp(range);
                    scan_comments(source, span, &mut view.html_comments);
                    view.html.push(span);
                    code.push(span);
                }
                Event::Code(text) => {
                    if let Some(h) = &mut heading {
                        h.text.push_str(&text);
                    }
                    code.push(sp(range));
                }
                Event::Text(text) => {
                    if let Some(h) = &mut heading {
                        h.text.push_str(&text);
                    }
                }
                _ => {}
            }
        }
        code.sort_unstable_by_key(|s| s.start);
        scan_imports(source, body_start, &code, &mut view.imports);
        view
    }

    /// Spans of fenced/indented code blocks, sorted (input for `estimate_markdown`).
    pub fn code_spans(&self) -> impl Iterator<Item = Span> + '_ {
        self.code_blocks.iter().map(|c| c.span)
    }
}

/// `(block span, YAML span)` when `source` opens with `---\n` (optional BOM, CRLF allowed)
/// and a later line is exactly `---`. Without a closing line there is no frontmatter.
fn split_frontmatter(source: &str) -> Option<(Span, Span)> {
    let start = if source.starts_with('\u{feff}') { 3 } else { 0 };
    let rest = source.get(start..)?;
    let yaml = rest
        .strip_prefix("---\n")
        .or_else(|| rest.strip_prefix("---\r\n"))?;
    let yaml_start = source.len() - yaml.len();
    let mut pos = yaml_start;
    for line in yaml.split_inclusive('\n') {
        let next = pos + line.len();
        if line.trim_end() == "---" {
            return Some((
                Span::new(to_u32(start), to_u32(next)),
                Span::new(to_u32(yaml_start), to_u32(pos)),
            ));
        }
        pos = next;
    }
    None
}

fn parse_frontmatter(source: &str, span: Span, body_span: Span, lines: &LineIndex) -> Frontmatter {
    let yaml = source
        .get(body_span.start as usize..body_span.end as usize)
        .unwrap_or_default();
    let first_line = lines.line_of(body_span.start);
    let value = match serde_yaml_ng::from_str::<Value>(yaml) {
        Ok(Value::Mapping(map)) => Ok(map),
        Ok(Value::Null) => Ok(Mapping::new()),
        Ok(_) => Err(YamlError {
            line: first_line,
            msg: "frontmatter is not a mapping".to_owned(),
        }),
        Err(e) => {
            let line = e.location().map_or(first_line, |l| {
                first_line + to_u32(l.line().saturating_sub(1))
            });
            Err(YamlError {
                line,
                msg: strip_mark(&e.to_string()),
            })
        }
    };
    let mut key_lines = BTreeMap::new();
    for (i, line) in yaml.lines().enumerate() {
        if let Some(key) = top_level_key(line) {
            key_lines
                .entry(key.to_owned())
                .or_insert(first_line + to_u32(i));
        }
    }
    Frontmatter {
        span,
        body_span,
        value,
        key_lines,
    }
}

/// Drops every YAML-relative ` at line N column M` mark; `YamlError::line` is file-relative.
fn strip_mark(msg: &str) -> String {
    const MARK: &str = " at line ";
    let mut out = String::with_capacity(msg.len());
    let mut rest = msg;
    while let Some(i) = rest.find(MARK) {
        out.push_str(&rest[..i]);
        let tail = &rest[i + MARK.len()..];
        let skip = tail
            .find(|c: char| !(c.is_ascii_digit() || c == ' ' || "column".contains(c)))
            .unwrap_or(tail.len());
        rest = &tail[skip..];
    }
    out.push_str(rest);
    out
}

/// `key` of a `key: …` line at column 0 (not a comment, list item or continuation).
fn top_level_key(line: &str) -> Option<&str> {
    let first = line.chars().next()?;
    if first.is_whitespace() || matches!(first, '#' | '-' | '}' | ']') {
        return None;
    }
    let (key, _) = line.split_once(':')?;
    let key = key.trim();
    let key = key
        .strip_prefix('"')
        .and_then(|k| k.strip_suffix('"'))
        .or_else(|| key.strip_prefix('\'').and_then(|k| k.strip_suffix('\'')))
        .unwrap_or(key);
    (!key.is_empty()).then_some(key)
}

/// Pushes every `<!-- … -->` inside `within`.
fn scan_comments(source: &str, within: Span, out: &mut Vec<Span>) {
    let bytes = source.as_bytes();
    let end = (within.end as usize).min(bytes.len());
    let mut pos = (within.start as usize).min(end);
    while let Some(i) = memchr::memmem::find(&bytes[pos..end], b"<!--") {
        let open = pos + i;
        let close =
            memchr::memmem::find(&bytes[open + 4..end], b"-->").map_or(end, |j| open + 4 + j + 3);
        out.push(Span::new(to_u32(open), to_u32(close)));
        pos = close;
    }
}

/// `@path` tokens (`@(\.{0,2}/)?[\w./-]+` at a word boundary) outside `code`, which is sorted.
/// Trailing dots are sentence punctuation, not path.
fn scan_imports(source: &str, body_start: usize, code: &[Span], out: &mut Vec<Import>) {
    let bytes = source.as_bytes();
    let mut code = code.iter().peekable();
    for at in memchr::memchr_iter(b'@', &bytes[body_start..]).map(|i| body_start + i) {
        while code.next_if(|s| (s.end as usize) <= at).is_some() {}
        if code.peek().is_some_and(|s| (s.start as usize) <= at) {
            continue;
        }
        let before = source[body_start..at].chars().next_back();
        // Word boundary (skips emails); `/`, `:`, `=` mean a bare URL or attribute.
        if before.is_some_and(|c| is_word(c) || matches!(c, '/' | ':' | '=')) {
            continue;
        }
        let rest = &source[at + 1..];
        let len = rest
            .char_indices()
            .find(|&(_, c)| !(is_word(c) || matches!(c, '.' | '/' | '-')))
            .map_or(rest.len(), |(i, _)| i);
        let path = rest[..len].trim_end_matches('.');
        if path.is_empty() || !path.chars().any(is_word) {
            continue;
        }
        out.push(Import {
            path: path.to_owned(),
            span: Span::new(to_u32(at), to_u32(at + 1 + path.len())),
        });
    }
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> MarkdownView {
        MarkdownView::parse(src, &LineIndex::new(src))
    }

    fn text(src: &str, span: Span) -> &str {
        &src[span.start as usize..span.end as usize]
    }

    /// Frontmatter + CRLF + code fences + comments + imports + links, all in one file.
    fn rich_sample() -> String {
        [
            "\u{feff}---",
            "description: Rich sample",
            "\"applyTo\": '**'",
            "tags:",
            "  - a",
            "  - nested: no",
            "---",
            "# Title `code`",
            "",
            "See @docs/guide.md and @../shared/rules.md. Mail me@example.com.",
            "",
            "<!-- hidden",
            "comment -->",
            "",
            "```bash title",
            "echo @not/an/import",
            "```",
            "",
            "Inline `@nope` and [link](https://x.dev) plus ![img](a.png) <!-- c2 -->.",
            "",
            "## Sub",
            "",
            "| a | b |",
            "|---|---|",
            "| ~~x~~ | y |",
            "",
        ]
        .join("\r\n")
    }

    #[test]
    fn rich_sample_spans_and_lines() {
        let src = rich_sample();
        let md = parse(&src);
        let fm = md.frontmatter.as_ref().expect("frontmatter");
        assert_eq!(fm.span.start, 3, "BOM is not part of the block");
        assert!(text(&src, fm.span).ends_with("---\r\n"));
        assert!(text(&src, fm.body_span).starts_with("description:"));
        assert_eq!(md.body.start, fm.span.end);
        assert_eq!(
            fm.get("description").and_then(Value::as_str),
            Some("Rich sample")
        );
        assert_eq!(fm.get("applyTo").and_then(Value::as_str), Some("**"));
        let mut keys: Vec<_> = fm.key_lines.iter().map(|(k, &l)| (k.as_str(), l)).collect();
        keys.sort_unstable();
        assert_eq!(keys, [("applyTo", 3), ("description", 2), ("tags", 4)]);

        let lines = LineIndex::new(&src);
        let heads: Vec<_> = md
            .headings
            .iter()
            .map(|h| (h.level, h.text.as_str(), lines.line_of(h.span.start)))
            .collect();
        assert_eq!(heads, [(1, "Title code", 8), (2, "Sub", 21)]);

        assert_eq!(md.code_blocks.len(), 1);
        let cb = &md.code_blocks[0];
        assert_eq!(cb.lang.as_deref(), Some("bash"));
        assert_eq!(cb.lines, (15, 17));
        assert!(text(&src, cb.span).starts_with("```bash"));

        let comments: Vec<_> = md.html_comments.iter().map(|&s| text(&src, s)).collect();
        assert_eq!(comments, ["<!-- hidden\r\ncomment -->", "<!-- c2 -->"]);
        assert_eq!(lines.line_of(md.html_comments[0].start), 12);

        let imports: Vec<_> = md
            .imports
            .iter()
            .map(|i| {
                (
                    i.path.as_str(),
                    text(&src, i.span),
                    lines.line_of(i.span.start),
                )
            })
            .collect();
        assert_eq!(
            imports,
            [
                ("docs/guide.md", "@docs/guide.md", 10),
                ("../shared/rules.md", "@../shared/rules.md", 10),
            ]
        );

        let links: Vec<_> = md
            .links
            .iter()
            .map(|l| (l.dest.as_str(), l.is_image, text(&src, l.span)))
            .collect();
        assert_eq!(
            links,
            [
                ("https://x.dev", false, "[link](https://x.dev)"),
                ("a.png", true, "![img](a.png)"),
            ]
        );
        let paras: Vec<_> = md
            .paragraphs
            .iter()
            .map(|&p| lines.line_of(p.start))
            .collect();
        assert_eq!(paras, [10, 19]);
    }

    #[test]
    fn tables_count_body_rows() {
        let src = "| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n\ntext\n\n| h |\n|---|\n";
        let md = parse(src);
        let rows: Vec<_> = md.tables.iter().map(|t| t.rows).collect();
        assert_eq!(rows, [2, 0]);
        assert!(text(src, md.tables[0].span).starts_with("| a |"));
    }

    #[test]
    fn raw_html_blocks_and_inline_tags() {
        let src = "<div hidden>\nx\n</div>\n\nText <span style=\"display:none\">y</span>.\n\n```\n<b>\n```\n";
        let md = parse(src);
        let html: Vec<_> = md.html.iter().map(|&s| text(src, s)).collect();
        assert_eq!(
            html,
            [
                "<div hidden>\nx\n</div>\n",
                "<span style=\"display:none\">",
                "</span>"
            ]
        );
    }

    #[test]
    fn frontmatter_invalid_yaml_reports_file_line() {
        let src = "---\nname: ok\ndescription: [unclosed\nother: 1\n---\nbody\n";
        let fm = parse(src).frontmatter.expect("frontmatter");
        let err = fm.value.expect_err("invalid yaml");
        // libyaml notices the unclosed flow sequence at the next key.
        assert_eq!(err.line, 4, "{err:?}");
        assert_eq!(
            err.msg,
            "did not find expected ',' or ']', while parsing a flow sequence"
        );
        assert_eq!(fm.key_lines.get("other"), Some(&4));
    }

    #[test]
    fn frontmatter_bad_indent_points_at_line() {
        let src = "---\na: 1\n b: 2\n---\n";
        let err = parse(src)
            .frontmatter
            .expect("frontmatter")
            .value
            .expect_err("bad");
        assert_eq!(err.line, 3, "{err:?}");
    }

    #[test]
    fn frontmatter_alias_bomb_is_an_error() {
        let mut src = String::from("---\na0: &a0 [x, x, x, x, x, x, x, x, x, x]\n");
        for i in 1..12 {
            let p = i - 1;
            src += &format!(
                "a{i}: &a{i} [*a{p}, *a{p}, *a{p}, *a{p}, *a{p}, *a{p}, *a{p}, *a{p}, *a{p}, *a{p}]\n"
            );
        }
        src += "---\n";
        let err = parse(&src)
            .frontmatter
            .expect("frontmatter")
            .value
            .expect_err("bomb");
        assert_eq!(err.msg, "repetition limit exceeded");
    }

    #[test]
    fn frontmatter_non_mapping_and_empty() {
        let fm = parse("---\n- a\n---\n").frontmatter.expect("frontmatter");
        assert_eq!(fm.value.expect_err("list").line, 2);
        let fm = parse("---\n---\nx").frontmatter.expect("frontmatter");
        assert_eq!(fm.value.expect("empty"), Mapping::new());
    }

    #[test]
    fn frontmatter_absent_without_opener_or_closer() {
        assert!(parse("--- \nx: 1\n---\n").frontmatter.is_none());
        assert!(parse("text\n---\nx: 1\n---\n").frontmatter.is_none());
        let md = parse("---\nnot closed\n");
        assert!(md.frontmatter.is_none());
        assert_eq!(md.body.start, 0);
    }

    #[test]
    fn imports_skip_emails_code_and_bare_at() {
        let md = parse(
            "a@b.c `@x` @ @. (@./local.md) @/abs/p @name. https://m.com/@u [@t](@d) <https://h/@a>\n\n    @indented\n",
        );
        let paths: Vec<_> = md.imports.iter().map(|i| i.path.as_str()).collect();
        assert_eq!(paths, ["./local.md", "/abs/p", "name"]);
    }

    #[test]
    fn plain_text_rules_file_parses() {
        let md = parse("Always use tabs.\nNever commit secrets.\n");
        assert!(md.frontmatter.is_none());
        assert_eq!(md.paragraphs.len(), 1);
        assert!(md.headings.is_empty());
    }
}
