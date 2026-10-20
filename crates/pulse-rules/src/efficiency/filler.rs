//! AI-EFF-007: filler phrasing.

use pulse_core::text::tokens;
use pulse_core::{Asset, Category, Finding, KindSet, Rule, RuleCtx, RuleMeta, Severity};

use crate::cognitive::directives::covers;
use crate::lexicon::style::FILLER_SET;
use crate::security::injection::at_word_bounds;
use crate::util::{Hit, aggregate};

pub static META: RuleMeta = RuleMeta {
    id: "AI-EFF-007",
    name: "Filler phrasing",
    category: Category::Efficiency,
    default_severity: Severity::Low,
    kinds: KindSet::MARKDOWN,
    summary: "Filler phrases spend tokens without adding instructions.",
    remediation: "Cut the filler: drop the phrase or use the shorter form suggested.",
    inline_suppressible: true,
};

pub struct FillerPhrasing;

impl Rule for FillerPhrasing {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, asset: &Asset, _ctx: &RuleCtx<'_>, out: &mut Vec<Finding>) {
        let norm = asset.normalized();
        let text = norm.text.as_str();
        let filler = &*FILLER_SET;
        let mut hits = Vec::new();
        for m in filler.automaton.find_iter(text) {
            if !at_word_bounds(text, m.range()) {
                continue;
            }
            let span = norm.source_span(m.range());
            if quoted(asset, span.start) {
                continue;
            }
            let id = m.pattern().as_usize();
            let cal = asset.calibration();
            let waste = tokens::estimate(&filler.display[id], cal)
                .saturating_sub(tokens::estimate(filler.replacement[id], cal));
            hits.push(Hit {
                span,
                quote: &filler.display[id],
                token_waste: waste,
            });
        }
        aggregate(
            &META,
            asset,
            &hits,
            |h| {
                let id = filler.display.iter().position(|d| d == h.quote);
                match id.map(|i| filler.replacement[i]).filter(|r| !r.is_empty()) {
                    Some(r) => format!("Filler \"{}\": write \"{r}\"", h.quote),
                    None => format!("Filler \"{}\": drop it", h.quote),
                }
            },
            |n, top| format!("{n} filler phrases: {top}"),
            out,
        );
    }
}

/// AI-EFF-007 guard: byte `start` is in a code block, an HTML comment or an inline code span
/// (odd number of backticks before it on its line), where a phrase is quoted, not written.
fn quoted(asset: &Asset, start: u32) -> bool {
    let pos = start as usize;
    let md = asset.markdown();
    if covers(&md.code_blocks, pos, |b| b.span) || covers(&md.html_comments, pos, |s| *s) {
        return true;
    }
    let line = asset
        .lines
        .line_span(&asset.source, asset.lines.line_of(start));
    asset
        .source
        .get(line.start as usize..pos)
        .is_some_and(|b| b.bytes().filter(|&c| c == b'`').count() % 2 == 1)
}
