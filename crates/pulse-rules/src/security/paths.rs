//! AI-SEC-013: references to credential stores and other sensitive paths.

use std::ops::Range;

use aho_corasick::Match;
use pulse_core::parse::HookFormat;
use pulse_core::{
    Asset, AssetKind, Category, Finding, KindSet, Rule, RuleCtx, RuleMeta, Severity, Span,
};

use crate::lexicon::paths::{DENY_CONTEXT, ENV_ID, ENV_TEMPLATES, GITIGNORE_WORDS, PATHS};
use crate::util::truncate;

/// Chars of a path quoted in a message.
const QUOTED: usize = 40;

pub static META: RuleMeta = RuleMeta {
    id: "AI-SEC-013",
    name: "Sensitive path reference",
    category: Category::Security,
    default_severity: Severity::Medium,
    kinds: KindSet::ALL,
    summary: "Asset references a credential store or private file (SSH keys, .env, cloud credentials).",
    remediation: "Remove the reference, or state it as a deny rule (`permissions.deny`, \"never read …\").",
    inline_suppressible: true,
};

pub struct SensitivePath;

impl Rule for SensitivePath {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, asset: &Asset, _ctx: &RuleCtx<'_>, out: &mut Vec<Finding>) {
        let src = asset.source.as_str();
        // Built on the first reference only: most assets have none.
        let mut filters = None;
        let mut last_token = usize::MAX..usize::MAX;
        for m in PATHS.find_iter(src) {
            if !is_reference(src, &m) {
                continue;
            }
            // AI-SEC-013 FP guard: a hook's trigger filter names the files that fire it.
            let at = u32::try_from(m.start()).unwrap_or(u32::MAX);
            let filters = filters.get_or_insert_with(|| hook_filters(asset));
            if filters.iter().any(|s| s.start <= at && at < s.end) {
                continue;
            }
            let token = token(src, m.range());
            // One finding per path token (`~/.ssh/id_rsa` holds two patterns).
            if token.start == last_token.start {
                continue;
            }
            last_token = token.clone();
            let span = span(token.clone());
            let quoted = src.get(token).unwrap_or_default();
            let mut f = Finding::new(
                &META,
                asset,
                span,
                format!("Sensitive path \"{}\" referenced", truncate(quoted, QUOTED)),
            );
            // AI-SEC-013 FP guard: deny lists name the paths they protect.
            let denied = asset.kind == AssetKind::Settings
                && asset
                    .settings()
                    .deny
                    .iter()
                    .any(|(_, s)| s.start <= span.start && span.end <= s.end);
            let in_text = if KindSet::MARKDOWN
                .with(AssetKind::Custom)
                .contains(asset.kind)
            {
                deny_context(src, m.start())
            } else {
                // JSON: only the string literal holding the path is prose.
                json_string(src, m.start()).is_some_and(|s| denies(sentence(s, 0)))
            };
            if denied || in_text {
                f.severity = Severity::Info;
            }
            out.push(f);
        }
    }
}

/// Trigger filters of a hook file: matchers, and the Kiro legacy `when` object (`patterns`).
fn hook_filters(asset: &Asset) -> Vec<Span> {
    if !matches!(asset.kind, AssetKind::Settings | AssetKind::Hooks) {
        return Vec::new();
    }
    let mut out: Vec<Span> = asset
        .hooks()
        .iter()
        .filter_map(|e| e.matcher_span)
        .collect();
    if asset.settings().hook_format == HookFormat::KiroLegacy
        && let Some(when) = asset.json().root.as_ref().and_then(|r| r.get("when"))
    {
        out.push(when.span);
    }
    out
}

/// Whether the match is a whole path fragment: not glued to an identifier
/// (`process.env`, `my_id_rsa_helper`), not a public key or an `.env` template, and not
/// `.env` in `.gitignore` advice.
pub(crate) fn is_reference(src: &str, m: &Match) -> bool {
    let b = src.as_bytes();
    let word = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    if m.start() > 0 && word(b[m.start() - 1]) {
        return false;
    }
    let rest = src.get(m.end()..).unwrap_or_default();
    let next = rest.bytes().next();
    let ends_word = src.as_bytes()[m.end() - 1].is_ascii_alphanumeric();
    if m.pattern().as_usize() != ENV_ID {
        return !(ends_word && next.is_some_and(word)) && !rest.starts_with(".pub");
    }
    if next.is_some_and(|c| word(c) || c == b'-') {
        return false;
    }
    if let Some(suffix) = rest.strip_prefix('.') {
        let s = suffix
            .split(|c: char| !c.is_ascii_alphanumeric())
            .next()
            .unwrap_or_default();
        if ENV_TEMPLATES.iter().any(|t| s.eq_ignore_ascii_case(t)) {
            return false;
        }
    }
    let line = line(src, m.start());
    let lower = src.get(line).unwrap_or_default().to_ascii_lowercase();
    !GITIGNORE_WORDS.iter().any(|w| lower.contains(w))
}

/// The path token around `r`, delimited by whitespace, quotes, brackets and separators;
/// always contains `r` (browser paths hold spaces).
fn token(src: &str, r: Range<usize>) -> Range<usize> {
    let delim = |c: char| {
        c.is_whitespace()
            || matches!(
                c,
                '"' | '\'' | '`' | '(' | ')' | '[' | ']' | '<' | '>' | ',' | ';' | '|'
            )
    };
    let start = src
        .get(..r.start)
        .and_then(|s| s.char_indices().rfind(|&(_, c)| delim(c)))
        .map_or(0, |(i, c)| i + c.len_utf8());
    let tail = src.get(r.end..).unwrap_or_default();
    let mut end = r.end + tail.find(delim).unwrap_or(tail.len());
    // Sentence punctuation after a path isn't part of it.
    while end > r.end
        && src
            .get(..end)
            .is_some_and(|s| s.ends_with(['.', ':', '!', '?']))
    {
        end -= 1;
    }
    start..end
}

/// The line holding byte `at`, without its newline.
fn line(src: &str, at: usize) -> Range<usize> {
    let start = src
        .get(..at)
        .and_then(|s| s.rfind('\n'))
        .map_or(0, |i| i + 1);
    let end = src
        .get(at..)
        .and_then(|s| s.find('\n'))
        .map_or(src.len(), |i| at + i);
    start..end
}

/// Whether byte `at` sits in a deny-list sentence (`Never read ~/.ssh`), or in a list
/// item whose lead-in line is one (`Do not access:` followed by `- .env`).
pub(crate) fn deny_context(src: &str, at: usize) -> bool {
    let r = line(src, at);
    let Some(text) = src.get(r.clone()) else {
        return false;
    };
    if denies(sentence(text, at - r.start)) {
        return true;
    }
    if !is_list_item(text) {
        return false;
    }
    // Walk up through the list to its lead-in.
    let mut end = r.start;
    while end > 0 {
        let prev = line(src, end - 1);
        let text = src.get(prev.clone()).unwrap_or_default();
        if text.trim().is_empty() || !is_list_item(text) {
            return !text.trim().is_empty() && denies(text);
        }
        end = prev.start;
    }
    false
}

/// The text of the JSON string literal holding byte `at`, from `at` back to its opening
/// quote and on to its closing one (escaped quotes end it early, which only shortens it).
fn json_string(src: &str, at: usize) -> Option<&str> {
    let r = line(src, at);
    let start = src.get(r.start..at)?.rfind('"')? + r.start + 1;
    let end = src.get(at..r.end)?.find('"')? + at;
    src.get(start..end)
}

/// The sentence of `line` around byte `at` (split on `. ` `! ` `? `).
fn sentence(line: &str, at: usize) -> &str {
    let stop = |s: &str| {
        s.char_indices()
            .find(|&(i, c)| matches!(c, '.' | '!' | '?') && s[i + 1..].starts_with(' '))
    };
    let before = line.get(..at).unwrap_or_default();
    let start = before
        .rmatch_indices(['.', '!', '?'])
        .find(|&(i, _)| before[i + 1..].starts_with(' '))
        .map_or(0, |(i, _)| i + 1);
    let after = line.get(at..).unwrap_or_default();
    let end = stop(after).map_or(line.len(), |(i, _)| at + i);
    line.get(start..end).unwrap_or(line)
}

pub(crate) fn denies(text: &str) -> bool {
    let lower = text.to_lowercase().replace('’', "'");
    DENY_CONTEXT.iter().any(|p| lower.contains(p))
}

fn is_list_item(line: &str) -> bool {
    let t = line.trim_start();
    if t.starts_with(['-', '*', '+']) {
        return true;
    }
    let digits = t.bytes().take_while(u8::is_ascii_digit).count();
    digits > 0 && t[digits..].starts_with(['.', ')'])
}

fn span(r: Range<usize>) -> Span {
    let c = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
    Span::new(c(r.start), c(r.end))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refs(src: &str) -> Vec<&str> {
        PATHS
            .find_iter(src)
            .filter(|m| is_reference(src, m))
            .map(|m| &src[token(src, m.range())])
            .collect()
    }

    #[test]
    fn references_and_lookalikes() {
        assert_eq!(
            refs("cat ~/.ssh/id_rsa."),
            ["~/.ssh/id_rsa", "~/.ssh/id_rsa"]
        );
        assert_eq!(refs("load `.env.local` first"), [".env.local"]);
        assert!(refs("read process.env.HOME and id_rsa.pub").is_empty());
        assert!(refs("copy .env.example to start").is_empty());
        assert!(refs("Add .env to .gitignore").is_empty());
        assert!(refs("see .envrc and my_keychain_x").is_empty());
    }

    #[test]
    fn deny_sentences_and_list_lead_ins() {
        let s = "Use the API. Never read ~/.ssh or .env. Read README.";
        assert!(deny_context(s, s.find(".ssh").unwrap_or(0)));
        assert!(!deny_context(s, s.find("README").unwrap_or(0)));
        let l = "Do not access these files:\n- ~/.aws/credentials\n- .env\n";
        assert!(deny_context(l, l.find(".env").unwrap_or(0)));
        let p = "Setup:\n- copy keys to ~/.ssh\n";
        assert!(!deny_context(p, p.find(".ssh").unwrap_or(0)));
    }
}
