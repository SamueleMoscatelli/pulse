//! Unstructured long instructions (AI-COG-007).

use pulse_core::{Asset, Category, Finding, KindSet, Rule, RuleCtx, RuleMeta, Severity};

pub static META: RuleMeta = RuleMeta {
    id: "AI-COG-007",
    name: "Unstructured long instructions",
    category: Category::Cognitive,
    default_severity: Severity::Low,
    kinds: KindSet::INSTRUCTIONS
        .union(KindSet::SCOPED_INSTRUCTIONS)
        .union(KindSet::AGENT)
        .union(KindSet::SKILL),
    summary: "Long instructions have too few headings, or a paragraph is a wall of text.",
    remediation: "Split the text into headed sections and short paragraphs or lists.",
    inline_suppressible: true,
};

/// Body lines above which at least [`MIN_HEADINGS`] headings are expected.
const MAX_FLAT_LINES: u32 = 60;
const MIN_HEADINGS: usize = 2;
/// Words allowed in one paragraph.
const MAX_PARAGRAPH_WORDS: usize = 150;

pub struct Unstructured;

impl Rule for Unstructured {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, asset: &Asset, _ctx: &RuleCtx<'_>, out: &mut Vec<Finding>) {
        let md = asset.markdown();
        let src = asset.source.as_str();
        if (md.body.start as usize) >= src.len() {
            return;
        }
        let first = asset.lines.line_of(md.body.start);
        let last = asset
            .lines
            .line_of(md.body.end.saturating_sub(1).max(md.body.start));
        let lines = last - first + 1;
        let headings = md.headings.len();
        if lines > MAX_FLAT_LINES && headings < MIN_HEADINGS {
            let span = asset.lines.line_span(src, first);
            let msg = format!(
                "{lines}-line body has {headings} heading(s); split it into titled sections"
            );
            out.push(Finding::new(&META, asset, span, msg));
        }
        // (count, longest, first) of the paragraphs over the limit.
        let mut long = (0usize, 0usize, None);
        for p in md.paragraphs.iter().filter(|p| p.start >= md.body.start) {
            let n = src
                .get(p.start as usize..p.end as usize)
                .unwrap_or_default()
                .split_whitespace()
                .count();
            if n > MAX_PARAGRAPH_WORDS {
                long.0 += 1;
                long.1 = long.1.max(n);
                long.2.get_or_insert(p.start);
            }
        }
        let (count, longest, Some(at)) = long else {
            return;
        };
        let span = asset.lines.line_span(src, asset.lines.line_of(at));
        let msg = if count == 1 {
            format!("Paragraph of {longest} words (> {MAX_PARAGRAPH_WORDS}); break it up")
        } else {
            format!(
                "{count} paragraphs over {MAX_PARAGRAPH_WORDS} words (longest {longest}); break them up"
            )
        };
        out.push(Finding::new(&META, asset, span, msg));
    }
}
