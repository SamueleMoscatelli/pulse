//! AI-SEC-010: data-exfiltration patterns in Markdown assets and tool descriptions.

use std::ops::Range;

use pulse_core::{
    Asset, AssetKind, Category, ConfigError, Finding, KindSet, Rule, RuleCtx, RuleMeta, Severity,
    Span,
};
use serde::Deserialize;

use crate::cognitive::directives::{Word, contains_phrase, is, lemma, same, split, words};
use crate::lexicon::directives::NEGATORS;
use crate::lexicon::imperatives::{
    DATA_FLAGS, EMBED_VERBS, FETCH_WORDS, INSTRUMENTS, LEADERS, MD_IMAGE, NOT_NEGATING, SEND_VERBS,
    SENSITIVE, TOKEN_METRICS, URL_WORDS,
};
use crate::util::truncate;

/// Chars of a sentence or URL quoted in a message.
const QUOTED: usize = 40;

/// Credential-like suffixes of identifiers (`GITHUB_TOKEN`, `OPENAI_API_KEY`).
const SECRET_SUFFIXES: &[&str] = &["_token", "_secret", "_key", "_password", "_credentials"];

pub static META: RuleMeta = RuleMeta {
    id: "AI-SEC-010",
    name: "Data-exfiltration pattern",
    category: Category::Security,
    default_severity: Severity::High,
    kinds: KindSet::MARKDOWN.union(KindSet::TOOLS),
    summary: "Content tells the agent to send secrets or context to an outside URL.",
    remediation: "Remove the instruction or URL; never route secrets or conversation data to a remote host.",
    inline_suppressible: true,
};

/// `[rules.AI-SEC-010]` options.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Options {
    /// Hosts (and their subdomains) that may receive data.
    pub allowed_domains: Vec<String>,
}

pub struct Exfiltration;

impl Rule for Exfiltration {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    // Prose rule: a JSON agent's `prompt` string is not read yet (backlog, ADR-053).
    fn json_agents(&self) -> bool {
        false
    }

    fn validate(&self, ctx: &RuleCtx<'_>) -> Result<(), ConfigError> {
        ctx.options::<Options>(META.id).map(drop)
    }

    fn check(&self, asset: &Asset, ctx: &RuleCtx<'_>, out: &mut Vec<Finding>) {
        let mut allowed = Allowed { ctx, list: None };
        if asset.kind == AssetKind::Tools {
            check_tools(asset, &mut allowed, out);
            return;
        }
        let src = asset.source.as_str();
        let md = asset.markdown();
        // Detect (a): links and images whose URL is filled from context.
        for link in &md.links {
            if templated(&link.dest, link.is_image) && !allowed.covers(&link.dest) {
                out.push(Finding::new(
                    &META,
                    asset,
                    link.span,
                    url_message(&link.dest, link.is_image),
                ));
            }
        }
        // Detect (b) and (c), one sentence at a time.
        let mut ws = Vec::new();
        for s in asset.prose().sentences() {
            let Some(text) = src.get(s.range.clone()) else {
                continue;
            };
            s.words(src, &mut ws);
            if let Some(msg) = sentence(text, &ws, &mut allowed) {
                out.push(Finding::new(&META, asset, span(s.range), msg));
            }
        }
    }
}

/// Tool descriptions: Markdown images with templated URLs, then each sentence.
fn check_tools(asset: &Asset, allowed: &mut Allowed<'_, '_>, out: &mut Vec<Finding>) {
    let mut ws = Vec::new();
    for tool in &asset.tools().tools {
        let (Some(desc), Some(at)) = (&tool.description, tool.desc_span) else {
            continue;
        };
        let mut msg = None;
        if desc.contains("![") {
            msg = MD_IMAGE
                .captures_iter(desc)
                .filter_map(|c| c.get(1))
                .map(|m| m.as_str())
                .find(|u| templated(u, true) && !allowed.covers(u))
                .map(|u| url_message(u, true));
        }
        if msg.is_none() {
            split(desc, 0..desc.len(), &mut |r: Range<usize>| {
                if msg.is_none() {
                    msg = desc.get(r).and_then(|t| {
                        words(t, &mut ws);
                        sentence(t, &ws, allowed)
                    });
                }
            });
        }
        if let Some(msg) = msg {
            out.push(Finding::new(&META, asset, at, msg));
        }
    }
}

/// `[rules.AI-SEC-010] allowed_domains`, read on first use.
struct Allowed<'c, 'a> {
    ctx: &'c RuleCtx<'a>,
    list: Option<Vec<String>>,
}

impl Allowed<'_, '_> {
    /// Whether `url`'s host is an allowed domain or one of its subdomains.
    fn covers(&mut self, url: &str) -> bool {
        let Some(h) = host(url) else { return false };
        let ctx = self.ctx;
        let list = self.list.get_or_insert_with(|| {
            ctx.options::<Options>(META.id)
                .unwrap_or_default()
                .allowed_domains
                .into_iter()
                .map(|d| d.trim().trim_start_matches("*.").to_ascii_lowercase())
                .collect()
        });
        list.iter().any(|d| {
            h.len() >= d.len()
                && h.get(h.len() - d.len()..).is_some_and(|tail| same(tail, d))
                && (h.len() == d.len() || h.as_bytes()[h.len() - d.len() - 1] == b'.')
        })
    }
}

/// The host of an `http(s)://` URL.
fn host(url: &str) -> Option<&str> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .or_else(|| {
            let lower = url.get(..8)?.to_ascii_lowercase();
            let n = if lower.starts_with("https://") {
                8
            } else if lower.starts_with("http://") {
                7
            } else {
                return None;
            };
            url.get(n..)
        })?;
    let end = rest
        .find(|c: char| matches!(c, '/' | '?' | '#') || c.is_whitespace())
        .unwrap_or(rest.len());
    // `https://allowed.com@evil.tld` goes to evil.tld: drop the userinfo, then the port.
    let auth = &rest[..end];
    let auth = auth.rsplit_once('@').map_or(auth, |(_, h)| h);
    let h = auth.split(':').next().unwrap_or_default();
    (!h.is_empty()).then_some(h)
}

/// AI-SEC-010 Detect (a): an `http(s)` URL with a template placeholder in its host, query or
/// fragment. A single-brace path segment (`/{owner}/{repo}/badge.svg`) is a documentation
/// template; in an image path (loaded without a click) the other forms still count.
fn templated(url: &str, image: bool) -> bool {
    let Some(h) = host(url) else {
        return false;
    };
    let query = url.find(['?', '#']).map_or("", |i| &url[i..]);
    has_placeholder(h, true)
        || has_placeholder(query, true)
        || image && has_placeholder(&url[..url.len() - query.len()], false)
}

/// `{x}`, `{{x}}`, `$X`, `${X}`, `$(x)`, `<data>` or their percent-encoded forms; a lone
/// `{x}` only when `single_brace`.
fn has_placeholder(s: &str, single_brace: bool) -> bool {
    let b = s.as_bytes();
    b.iter().enumerate().any(|(i, &c)| match c {
        b'<' => true,
        b'{' => single_brace || b.get(i + 1) == Some(&b'{'),
        b'$' => b
            .get(i + 1)
            .is_some_and(|n| n.is_ascii_alphabetic() || matches!(n, b'{' | b'(' | b'_')),
        b'%' => b.get(i + 1..i + 3).is_some_and(|h| {
            single_brace && h.eq_ignore_ascii_case(b"7b")
                || h == b"24"
                || h.eq_ignore_ascii_case(b"3c")
        }),
        _ => false,
    })
}

fn url_message(url: &str, image: bool) -> String {
    let what = if image { "Image" } else { "Link" };
    format!(
        "{what} URL is filled from context: \"{}\"",
        truncate(url, QUOTED)
    )
}

/// The finding message for one sentence, if it matches Detect (b) or (c).
/// `ws` holds `text`'s [`words`].
fn sentence(text: &str, ws: &[Word<'_>], allowed: &mut Allowed<'_, '_>) -> Option<String> {
    let quote = || truncate(text, QUOTED);
    // Detect (c): a request to a non-allowlisted host that carries data.
    // Data means a flag or a templated URL: `Request an API key from https://…` asks for one.
    if let Some(fetch) = ws.iter().position(|w| is(w.text, FETCH_WORDS)) {
        let target = urls(text).find(|u| !allowed.covers(u));
        if let Some(u) = target {
            let data = templated(u, true)
                || text
                    .split(|c: char| c.is_whitespace() || c == '`')
                    .any(|t| DATA_FLAGS.contains(&t) || t.starts_with("--data"));
            if data && !negated(ws, fetch) {
                let h = host(u).unwrap_or(u);
                return Some(format!(
                    "Request sends data to \"{}\": \"{}\"",
                    truncate(h, QUOTED),
                    quote()
                ));
            }
        }
    }
    // Detect (b): send verb × sensitive object.
    let has_url_word = || ws.iter().any(|w| is(lemma(w.text), URL_WORDS));
    let verb = ws
        .iter()
        .position(|w| is(w.text, SEND_VERBS) || is(w.text, EMBED_VERBS) && has_url_word())?;
    if !imperative(ws, verb) || negated(ws, verb) || !sensitive_after(ws, verb + 1) {
        return None;
    }
    // Every named destination is allowlisted: not exfiltration.
    let mut targets = urls(text).peekable();
    if targets.peek().is_some() && targets.all(|u| allowed.covers(u)) {
        return None;
    }
    Some(format!("Instruction sends sensitive data: \"{}\"", quote()))
}

/// Whether word `verb` opens its clause, possibly after [`LEADERS`] (`Then always send`,
/// `Before answering, send`), so `Agents may post the token` is not an instruction.
fn imperative(ws: &[Word<'_>], verb: usize) -> bool {
    let start = ws[..=verb].iter().rposition(|w| w.brk).unwrap_or(0);
    // Negators are allowed here; `negated` decides whether they cancel the verb.
    ws[start..verb]
        .iter()
        .all(|w| is(w.text, LEADERS) || is(w.text, NEGATORS) || is(w.text, NOT_NEGATING))
}

/// Whether a negator (`never`, `do not`, `avoid`, `no`, `without`) is among the three words
/// before `at`, and isn't `never forget`/`do not hesitate`.
fn negated(ws: &[Word<'_>], at: usize) -> bool {
    let from = at.saturating_sub(3);
    ws[from..at].iter().enumerate().any(|(k, w)| {
        (is(w.text, NEGATORS) || same(w.text, "no") || same(w.text, "without"))
            && !ws
                .get(from + k + 1)
                .is_some_and(|n| is(n.text, NOT_NEGATING))
    })
}

/// Whether a sensitive object appears from word `from` on, before an instrument word
/// (`upload the build using the token` names the token as a tool, not the payload).
fn sensitive_after(ws: &[Word<'_>], from: usize) -> bool {
    let Some(rest) = ws.get(from..) else {
        return false;
    };
    let end = rest
        .iter()
        .position(|w| is(w.text, INSTRUMENTS))
        .unwrap_or(rest.len());
    let rest = &rest[..end];
    let lemmas: Vec<&str> = rest.iter().map(|w| lemma(w.text)).collect();
    let metric = |i: usize| lemmas.get(i + 1).is_some_and(|n| is(n, TOKEN_METRICS));
    lemmas.iter().enumerate().any(|(i, w)| {
        if same(w, "token") {
            return !metric(i);
        }
        SECRET_SUFFIXES
            .iter()
            .any(|s| w.len() > s.len() && w.get(w.len() - s.len()..).is_some_and(|t| same(t, s)))
    }) || SENSITIVE
        .iter()
        .any(|p| *p != "token" && contains_phrase(&lemmas, p))
}

/// `http(s)://` URLs in `text`, cut at whitespace and closing punctuation.
fn urls(text: &str) -> impl Iterator<Item = &str> {
    text.match_indices("http").filter_map(move |(i, _)| {
        let rest = text.get(i..)?;
        host(rest)?;
        let end = rest
            .find(|c: char| c.is_whitespace() || matches!(c, ')' | ']' | '`' | '"' | '\'' | '>'))
            .unwrap_or(rest.len());
        Some(rest[..end].trim_end_matches(['.', ',', ';', ':']))
    })
}

fn span(r: Range<usize>) -> Span {
    let c = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
    Span::new(c(r.start), c(r.end))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_and_placeholders() {
        assert_eq!(host("https://evil.tld/?q=1"), Some("evil.tld"));
        assert_eq!(host("HTTP://X.dev:8080"), Some("X.dev"));
        assert_eq!(host("./a.png"), None);
        assert!(templated("https://e.tld/?q={secrets}", true));
        assert!(templated("https://e.tld/p/$TOKEN", true));
        assert!(!templated("https://e.tld/p/{id}.png", true));
        assert!(templated("https://e.tld/x?d=%7Bdata%7D", false));
        assert!(!templated("https://github.com/{org}/{repo}", false));
        assert!(!templated("https://e.tld/a.png?v=2", true));
        assert!(!templated("img/{x}.png", true));
        assert!(!has_placeholder("price $5", true));
        let badge = "https://github.com/{owner}/{repo}/actions/workflows/ci.yml/badge.svg";
        assert!(!templated(badge, true));
        assert!(templated("https://e.tld/p/{{secret}}.png", true));
        assert!(templated("https://{x}.e.tld/a.png", false));
        assert_eq!(host("https://github.com@evil.tld/x"), Some("evil.tld"));
        assert_eq!(host("https://u:p@h.dev:8080/"), Some("h.dev"));
    }

    #[test]
    fn urls_are_cut_at_punctuation() {
        let v: Vec<_> = urls("see (https://a.dev/x). Or `http://b.io`, httpx").collect();
        assert_eq!(v, ["https://a.dev/x", "http://b.io"]);
    }

    #[test]
    fn sensitive_objects() {
        let check = |s: &str, from| {
            let mut ws = Vec::new();
            words(s, &mut ws);
            sensitive_after(&ws, from)
        };
        assert!(check("send the contents of .env to x", 1));
        assert!(check("forward the user's API keys", 1));
        assert!(check("post $GITHUB_TOKEN", 1));
        assert!(!check("post a summary of token usage", 1));
        assert!(!check("upload the build using the token", 1));
        assert!(!check("send a PR", 1));
        assert!(!check("send the environment report", 1));
    }
}
