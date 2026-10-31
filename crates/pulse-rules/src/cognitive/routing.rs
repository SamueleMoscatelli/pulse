//! Weak routing description (AI-COG-006).

use pulse_core::{
    Asset, AssetKind, Category, Finding, KindSet, Rule, RuleCtx, RuleMeta, Severity, Span,
};

use super::directives::{Word, is, lemma, same, words};
use crate::lexicon::routing::{
    ARTICLES, CUE_PREFIXES, FIRST_PERSON, USE_FORMS, USE_SCOPES, USE_WINDOW, WHEN,
};
use crate::util::truncate;

/// The shape of a description that routes well.
macro_rules! template {
    () => {
        "Template: \"<What it does>. Use when <requests, keywords, file types>. \
         [Not for <what a sibling handles>.]\""
    };
}

pub static META: RuleMeta = RuleMeta {
    id: "AI-COG-006",
    name: "Weak routing description",
    category: Category::Cognitive,
    default_severity: Severity::Medium,
    // Scoped instructions: only Kiro `inclusion: auto` steering, which routes on its description.
    kinds: KindSet::SKILL
        .union(KindSet::AGENT)
        .union(KindSet::SCOPED_INSTRUCTIONS),
    summary: "A skill, agent or auto steering description is too short, lacks a trigger cue, or is in first person.",
    remediation: concat!(
        "Say what it does and when to use it, in the third person. ",
        template!()
    ),
    inline_suppressible: true,
};

/// Fewest words in a description.
const MIN_WORDS: usize = 20;

pub struct WeakRouting;

impl Rule for WeakRouting {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    // Kiro JSON agents: the `description` string.
    fn json_agents(&self) -> bool {
        true
    }

    fn check(&self, asset: &Asset, _ctx: &RuleCtx<'_>, out: &mut Vec<Finding>) {
        if asset.kind == AssetKind::ScopedInstructions && asset.kiro_inclusion() != Some("auto") {
            return;
        }
        // A missing or empty description is AI-STR-001's.
        let Some((desc, span)) = description(asset).filter(|(d, _)| !d.trim().is_empty()) else {
            return;
        };
        let mut ws = Vec::new();
        words(desc, &mut ws);
        let short = ws.len() < MIN_WORDS;
        let no_cue = !has_cue(desc, &ws);
        let first_person = first_person(desc, &ws);
        if !short && !no_cue && !first_person {
            return;
        }
        let (mut why, mut fix) = (Vec::new(), Vec::new());
        if short {
            let n = ws.len();
            let s = if n == 1 { "" } else { "s" };
            why.push(format!("{n} word{s} (< {MIN_WORDS})"));
            fix.push("say what it covers");
        }
        if no_cue {
            why.push("no trigger cue".into());
            fix.push(
                "add \"Use when …\" with the requests, keywords and file types that should trigger it",
            );
        }
        if first_person {
            why.push("first person".into());
            fix.push("write it in the third person (\"Reviews …\", not \"I review …\")");
        }
        let msg = format!(
            "Weak routing description \"{}\": {}",
            truncate(desc.trim(), 24),
            why.join(", ")
        );
        let fix = format!(concat!("{}. ", template!()), capitalize(&fix.join("; ")));
        out.push(Finding::new(&META, asset, span, msg).with_fix(fix));
    }
}

/// The description and the span reported for it: a JSON agent's `description` string, else
/// the frontmatter key's line.
fn description(asset: &Asset) -> Option<(&str, Span)> {
    if asset.is_json_agent() {
        let (d, span) = asset.kiro_agent()?.description.as_ref()?;
        return Some((d, *span));
    }
    let fm = asset.markdown().frontmatter.as_ref()?;
    let desc = fm.get("description")?.as_str()?;
    let span = fm
        .key_lines
        .get("description")
        .map_or(fm.span, |&l| asset.lines.line_span(&asset.source, l));
    Some((desc, span))
}

/// Whether the words of `desc` carry an AI-COG-006 trigger cue: `trigger*`/`invok*`/
/// `proactive*`, a sentence opening with `Use` (`Use after a change`), `use … when|for`,
/// or `when the user`.
fn has_cue(desc: &str, w: &[Word<'_>]) -> bool {
    w.iter().enumerate().any(|(i, x)| {
        let t = x.text;
        let rest = &w[i + 1..];
        CUE_PREFIXES.iter().any(|p| starts_with_ci(t, p))
            || is(t, USE_FORMS)
                && (opens_sentence(desc, t)
                    || rest.iter().take(USE_WINDOW).any(|n| is(n.text, USE_SCOPES)))
            || is(t, WHEN)
                && rest
                    .iter()
                    .find(|n| !is(n.text, ARTICLES))
                    .is_some_and(|n| same(lemma(n.text), "user"))
    })
}

/// Whether `word` (a slice of `desc`) starts a sentence.
fn opens_sentence(desc: &str, word: &str) -> bool {
    desc.get(..offset(desc, word))
        .unwrap_or_default()
        .trim_end()
        .chars()
        .next_back()
        .is_none_or(|c| matches!(c, '.' | '!' | '?'))
}

/// Byte offset of `word` in `desc`; `word` must be a sub-slice of `desc` (as `words` returns).
fn offset(desc: &str, word: &str) -> usize {
    (word.as_ptr() as usize).saturating_sub(desc.as_ptr() as usize)
}

/// Whether a first-person word (`I`, `I'm`, …) occurs outside quotes and code spans.
fn first_person(desc: &str, w: &[Word<'_>]) -> bool {
    w.iter().any(|x| {
        FIRST_PERSON.contains(&x.text)
            && !quoted(desc.get(..offset(desc, x.text)).unwrap_or_default())
    })
}

/// Whether text ending in `before` leaves a quote or code span open.
fn quoted(before: &str) -> bool {
    let n = |c: char| before.chars().filter(|&x| x == c).count();
    n('"') % 2 == 1 || n('`') % 2 == 1 || n('“') > n('”')
}

/// `s` with its first letter in upper case.
fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

fn starts_with_ci(w: &str, prefix: &str) -> bool {
    w.get(..prefix.len())
        .is_some_and(|h| h.eq_ignore_ascii_case(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cue(s: &str) -> bool {
        let mut w = Vec::new();
        words(s, &mut w);
        has_cue(s, &w)
    }

    fn fp(s: &str) -> bool {
        let mut w = Vec::new();
        words(s, &mut w);
        first_person(s, &w)
    }

    #[test]
    fn trigger_cues() {
        assert!(cue("Use when the user asks to deploy"));
        assert!(cue("Use this agent for reviewing diffs"));
        assert!(cue("Should be used whenever tests fail"));
        assert!(cue("Runs when a user mentions PDFs"));
        assert!(cue("Use proactively after edits"));
        assert!(cue("Triggers on 'deploy'"));
        assert!(cue("Invoke to format code"));
        assert!(!cue("Reviews code for bugs and style"));
        assert!(cue("Reviews diffs. Use at the end of a task, after tests"));
        assert!(!cue("Uses Python for parsing"));
        assert!(!cue("Tools to use: grep, sed"));
        assert!(!cue("When done, report back"));
    }

    #[test]
    fn first_person_outside_quotes() {
        assert!(fp("I can review your code"));
        assert!(fp("Reviews code. I'll report bugs"));
        assert!(!fp("Use when the user says \"I need a review\""));
        assert!(!fp("Use when the user says “I'm stuck”"));
        assert!(!fp("Compute i for each row"));
    }

    #[test]
    fn prefix_is_char_boundary_safe() {
        assert!(!starts_with_ci("é", "trigger"));
        assert!(!starts_with_ci("triég", "trigger"));
    }
}
