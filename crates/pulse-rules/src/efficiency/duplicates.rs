//! AI-EFF-003: paragraphs repeated (or lightly reworded) within and across assets, and exact
//! duplicate lines across always-on assets. Project scope; MinHash LSH avoids O(n²) (ADR-029).

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

use rayon::prelude::*;

use pulse_core::text::shingles::{self, SIG_LEN, Signature};
use pulse_core::text::{Normalized, tokens};
use pulse_core::{
    Asset, AssetKind, Category, Finding, KindSet, LoadMode, ProjectRule, RuleCtx, RuleMeta,
    Severity, Span,
};

use crate::util::{tail, truncate};

pub static META: RuleMeta = RuleMeta {
    id: "AI-EFF-003",
    name: "Duplicated content",
    category: Category::Efficiency,
    default_severity: Severity::Medium,
    kinds: KindSet::MARKDOWN,
    summary: "The same paragraph or line appears more than once in the context.",
    remediation: "Keep one copy and reference it (an @import or a link) from the other places.",
    inline_suppressible: true,
};

/// Paragraphs shorter than this are not compared.
const MIN_PARA_WORDS: usize = 25;
/// Lines shorter than this are not compared.
const MIN_LINE_WORDS: usize = 8;
/// Signature bins that must agree (of [`SIG_LEN`]): estimated Jaccard ≥ 0.5.
const MIN_AGREEMENT: usize = SIG_LEN / 2;
/// LSH bands, one signature bin each: a pair at Jaccard 0.5 shares one with p ≈ 1 − 0.5⁸.
const BANDS: usize = 8;
/// Earlier bucket members examined per band (newest first), bounding pathological buckets.
const MAX_CHAIN: usize = 64;
/// Chars of the earlier occurrence's path quoted in a message (its tail is kept).
const QUOTED_PATH: usize = 24;
/// Repeated lines listed in a message.
const LISTED_LINES: usize = 3;
/// Chars of the duplicate quoted in a message.
const QUOTED_TEXT: usize = 40;

pub struct DuplicatedContent;

impl ProjectRule for DuplicatedContent {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_project(&self, assets: &[Asset], ctx: &RuleCtx<'_>, out: &mut Vec<Finding>) {
        let mut ordered: Vec<&Asset> = assets
            .iter()
            .filter(|a| META.kinds.contains(a.kind) && a.skipped.is_none())
            .collect();
        // "Earlier" is earlier in path order, whatever order discovery produced.
        ordered.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
        let roots: Vec<&str> = ordered
            .iter()
            .map(|a| context_root(&a.rel_path, a.kind))
            .collect();
        let prints: Vec<Prints> = ordered
            .par_iter()
            .map_init(Words::default, |scratch, a| Prints::of(a, scratch))
            .collect();
        let calibration = ctx.config.context.calibration;
        let flagged = paragraphs(&ordered, &roots, &prints, calibration, out);
        lines(&ordered, &roots, &prints, &flagged, calibration, out);
    }
}

/// The comparable units of one asset, computed in parallel.
struct Prints {
    /// Paragraphs of ≥ [`MIN_PARA_WORDS`] words, in source order.
    /// With their signature and exact word-sequence hash.
    paras: Vec<(Span, Signature, u64)>,
    /// Sequence hashes of always-on lines of ≥ [`MIN_LINE_WORDS`] words outside fenced
    /// code and frontmatter, in source order.
    lines: Vec<(u64, Span)>,
}

impl Prints {
    /// Hashes the asset's words once; paragraphs and lines are slices of them.
    fn of(asset: &Asset, scratch: &mut Words) -> Self {
        let md = asset.markdown();
        let norm = asset.normalized();
        scratch.starts.clear();
        scratch.hashes.clear();
        shingles::word_hashes_at(&norm.text, &mut scratch.starts, &mut scratch.hashes);
        let words = |span: Span| {
            let (a, b) = norm_range(norm, span);
            let first = scratch.starts.partition_point(|&s| (s as usize) < a);
            let end = scratch.starts.partition_point(|&s| (s as usize) < b);
            &scratch.hashes[first..end.max(first)]
        };
        let mut paras = Vec::new();
        for &span in &md.paragraphs {
            let w = words(span);
            if w.len() >= MIN_PARA_WORDS {
                paras.push((span, shingles::minhash(w), shingles::sequence_hash(w)));
            }
        }
        let mut lines = Vec::new();
        if always_on(asset) {
            let mut code = md.code_spans().peekable();
            for line in 1..=asset.lines.line_count() as u32 {
                let span = asset.lines.line_span(&asset.source, line);
                if span.start < md.body.start || span.start == span.end || covered(&mut code, span)
                {
                    continue;
                }
                let w = words(span);
                if w.len() >= MIN_LINE_WORDS {
                    lines.push((shingles::sequence_hash(w), span));
                }
            }
        }
        Self { paras, lines }
    }
}

/// Per-thread buffers: the word offsets and hashes of one asset's normalized text.
#[derive(Default)]
struct Words {
    starts: Vec<u32>,
    hashes: Vec<u64>,
}

/// Reports near-duplicate paragraphs; returns the flagged spans per asset (sorted).
fn paragraphs(
    assets: &[&Asset],
    roots: &[&str],
    prints: &[Prints],
    calibration: f64,
    out: &mut Vec<Finding>,
) -> Vec<Vec<Span>> {
    // (asset, span, signature) of every paragraph, in path then source order.
    let paras: Vec<(usize, Span, &Signature, u64)> = prints
        .iter()
        .enumerate()
        .flat_map(|(i, p)| {
            p.paras
                .iter()
                .map(move |(span, sig, seq)| (i, *span, sig, *seq))
        })
        .collect();
    let mut flagged = vec![Vec::new(); assets.len()];
    // Bucket heads by (band, bin value), and `prev[node]`: the previous node of a bucket,
    // newest first. Node `i * BANDS + b` is paragraph `i` in band `b`.
    let mut heads: HashMap<u64, u32, IdBuild> =
        HashMap::with_capacity_and_hasher(paras.len() * BANDS, IdBuild::default());
    let mut prev = vec![u32::MAX; paras.len() * BANDS];
    for (i, &(asset, span, sig, seq)) in paras.iter().enumerate() {
        // The nearest earlier similar paragraph in a related context, across all bands.
        let mut best: Option<usize> = None;
        for (b, &value) in sig.iter().take(BANDS).enumerate() {
            let node = i * BANDS + b;
            let key = ((b as u64) << 32) | u64::from(value);
            let mut next = heads.insert(key, node as u32).unwrap_or(u32::MAX);
            prev[node] = next;
            for _ in 0..MAX_CHAIN {
                let j = next as usize / BANDS;
                let Some(&(other, _, other_sig, _)) = paras.get(j) else {
                    break;
                };
                if best.is_some_and(|best| j <= best) {
                    break;
                }
                if (other == asset || related(roots[other], roots[asset]))
                    && shingles::agreement(other_sig, sig) >= MIN_AGREEMENT
                {
                    best = Some(j);
                    break;
                }
                next = prev[next as usize];
            }
        }
        let Some((other, other_span, _, other_seq)) = best.map(|j| paras[j]) else {
            continue;
        };
        let what = if other_seq == seq {
            "Duplicate paragraph"
        } else {
            "Near-duplicate paragraph"
        };
        out.push(finding(
            assets[asset],
            assets[other],
            span,
            other_span,
            what,
            calibration,
        ));
        flagged[asset].push(span);
    }
    flagged
}

/// Reports lines repeated word for word in another always-on asset of a related context.
fn lines(
    assets: &[&Asset],
    roots: &[&str],
    prints: &[Prints],
    flagged: &[Vec<Span>],
    calibration: f64,
    out: &mut Vec<Finding>,
) {
    // (asset, sequence hash, span) of every line outside a reported paragraph.
    let mut all: Vec<(usize, u64, Span)> = Vec::new();
    for (i, p) in prints.iter().enumerate() {
        let mut skip = flagged[i].iter().copied().peekable();
        all.extend(
            p.lines
                .iter()
                .filter(|&&(_, span)| !covered(&mut skip, span))
                .map(|&(key, span)| (i, key, span)),
        );
    }
    // Newest-first chains of equal lines, as for paragraphs: an unrelated copy (a sibling
    // package) must not hide a related one.
    let mut heads: HashMap<u64, u32, IdBuild> =
        HashMap::with_capacity_and_hasher(all.len(), IdBuild::default());
    let mut prev = vec![u32::MAX; all.len()];
    // (asset, earlier asset, line span, earlier line span) of every repeated line.
    let mut pairs: Vec<(usize, usize, Span, Span)> = Vec::new();
    for (n, &(i, key, span)) in all.iter().enumerate() {
        let mut next = heads.insert(key, n as u32).unwrap_or(u32::MAX);
        prev[n] = next;
        for _ in 0..MAX_CHAIN {
            let Some(&(j, _, earlier)) = all.get(next as usize) else {
                break;
            };
            if j != i && related(roots[j], roots[i]) {
                pairs.push((i, j, trimmed(assets[i], span), earlier));
                break;
            }
            next = prev[next as usize];
        }
    }
    // One finding per file pair, at its first line.
    pairs.sort_by_key(|&(i, j, span, _)| (i, j, span.start));
    for group in pairs.chunk_by(|a, b| (a.0, a.1) == (b.0, b.1)) {
        let (i, j) = (group[0].0, group[0].1);
        out.push(line_finding(assets[i], assets[j], group, calibration));
    }
}

/// The AI-EFF-003 finding for the lines of `asset` repeated in `earlier` (one pair, sorted).
fn line_finding(
    asset: &Asset,
    earlier: &Asset,
    group: &[(usize, usize, Span, Span)],
    calibration: f64,
) -> Finding {
    let waste: u32 = group
        .iter()
        .map(|&(_, _, span, _)| {
            let text = asset
                .source
                .get(span.start as usize..span.end as usize)
                .unwrap_or_default();
            tokens::estimate(text, calibration)
        })
        .sum();
    let list = |a: &Asset, spans: &mut dyn Iterator<Item = Span>| {
        let mut lines: Vec<String> = spans
            .take(LISTED_LINES + 1)
            .map(|s| a.lines.line_of(s.start).to_string())
            .collect();
        if lines.len() > LISTED_LINES {
            lines[LISTED_LINES] = "…".into();
        }
        lines.join(", ")
    };
    let theirs = list(earlier, &mut group.iter().map(|g| g.3));
    let msg = if group.len() == 1 {
        format!(
            "Line also in `{}:{theirs}`; both load on every request, \
             so it is paid twice (~{waste} tokens)",
            earlier.rel_path
        )
    } else {
        format!(
            "Lines {} also in `{}:{theirs}`; both load on every request, \
             so they are paid twice (~{waste} tokens)",
            list(asset, &mut group.iter().map(|g| g.2)),
            earlier.rel_path
        )
    };
    let mut f = Finding::new(&META, asset, group[0].2, msg);
    // Repeated lines cost less than repeated paragraphs (P8.7).
    f.severity = Severity::Low;
    f.token_waste = waste;
    f
}

fn finding(
    asset: &Asset,
    earlier: &Asset,
    span: Span,
    earlier_span: Span,
    what: &str,
    calibration: f64,
) -> Finding {
    let text = asset
        .source
        .get(span.start as usize..span.end as usize)
        .unwrap_or_default();
    let snippet = text.lines().next().unwrap_or_default().trim();
    let line = earlier.lines.line_of(earlier_span.start);
    let mut f = Finding::new(
        &META,
        asset,
        span,
        format!(
            "{what} of {}:{line}: \"{}\"",
            tail(&earlier.rel_path, QUOTED_PATH),
            truncate(snippet, QUOTED_TEXT)
        ),
    );
    f.token_waste = tokens::estimate(text, calibration);
    f
}

/// Whether `span` starts inside one of the sorted, non-overlapping `spans` (consumed
/// past `span`).
fn covered(spans: &mut std::iter::Peekable<impl Iterator<Item = Span>>, span: Span) -> bool {
    while let Some(s) = spans.peek() {
        if s.end <= span.start {
            spans.next();
        } else {
            return s.start <= span.start;
        }
    }
    false
}

/// `span` without leading and trailing whitespace.
fn trimmed(asset: &Asset, span: Span) -> Span {
    let text = asset
        .source
        .get(span.start as usize..span.end as usize)
        .unwrap_or_default();
    let lead = (text.len() - text.trim_start().len()) as u32;
    let trail = (text.len() - text.trim_end().len()) as u32;
    Span::new(span.start + lead, span.end - trail)
}

/// Assets whose whole body is in every session (skills and agents only add a description).
fn always_on(asset: &Asset) -> bool {
    asset.effective_load() == LoadMode::Always
        && !matches!(asset.kind, AssetKind::Skill | AssetKind::Agent)
}

/// The normalized byte range of source `span`.
fn norm_range(norm: &Normalized, span: Span) -> (usize, usize) {
    let a = norm.map.partition_point(|&m| m < span.start);
    let b = norm.map.partition_point(|&m| m < span.end);
    (a.min(norm.text.len()), b.min(norm.text.len()))
}

/// The directory whose sessions load this asset: the part before a platform directory
/// (`pkg/.claude/agents/x.md` → `pkg`), the folder holding `skills/` for a skill, else the
/// asset's directory. FP guard: monorepo packages that are never loaded together.
fn context_root(rel: &str, kind: AssetKind) -> &str {
    let mut offset = 0;
    for seg in rel.split('/') {
        if seg.starts_with('.') && offset + seg.len() < rel.len() {
            return rel[..offset].trim_end_matches('/');
        }
        offset += seg.len() + 1;
    }
    let dir = parent(rel);
    if kind == AssetKind::Skill {
        let up = parent(dir);
        if up == "skills" || up.ends_with("/skills") {
            return parent(up);
        }
        return up;
    }
    dir
}

fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(d, _)| d)
}

/// Whether one context root contains the other (both may be loaded in one session).
fn related(a: &str, b: &str) -> bool {
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    short.is_empty()
        || long
            .strip_prefix(short)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

/// Keys are already uniform hashes: mix them once instead of re-hashing.
#[derive(Default)]
struct IdHasher(u64);

impl Hasher for IdHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 ^ u64::from(b)).wrapping_mul(0x0100_0000_01B3);
        }
    }

    fn write_u64(&mut self, n: u64) {
        self.0 = n.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    }
}

type IdBuild = BuildHasherDefault<IdHasher>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_roots() {
        assert_eq!(context_root("CLAUDE.md", AssetKind::Instructions), "");
        assert_eq!(
            context_root("pkg/a/AGENTS.md", AssetKind::Instructions),
            "pkg/a"
        );
        assert_eq!(
            context_root("pkg/.claude/agents/r.md", AssetKind::Agent),
            "pkg"
        );
        assert_eq!(
            context_root(".github/copilot-instructions.md", AssetKind::Instructions),
            ""
        );
        assert_eq!(
            context_root("pkg/skills/rel/SKILL.md", AssetKind::Skill),
            "pkg"
        );
        assert_eq!(context_root(".hidden.md", AssetKind::Instructions), "");
    }

    #[test]
    fn related_contexts_nest() {
        assert!(related("", "pkg/a"));
        assert!(related("pkg", "pkg/a"));
        assert!(related("pkg/a", "pkg/a"));
        assert!(!related("pkg/a", "pkg/b"));
        assert!(!related("pkg/a", "pkg/ab"));
    }
}
