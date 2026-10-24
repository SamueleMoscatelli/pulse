//! AI-SEC-003: hardcoded secrets.

use pulse_core::{Asset, Category, Finding, KindSet, Rule, RuleCtx, RuleMeta, Severity, Span};

use crate::lexicon::secrets::{MATCHERS, PROVIDERS, is_placeholder, plausible};

pub static META: RuleMeta = RuleMeta {
    id: "AI-SEC-003",
    name: "Hardcoded secret",
    category: Category::Security,
    default_severity: Severity::Critical,
    kinds: KindSet::ALL,
    summary: "File contains what looks like a real API key, token, password or private key.",
    remediation: "Revoke the secret, then reference it from the environment (e.g. ${env:API_KEY}).",
    inline_suppressible: false,
};

/// Characters of a secret shown in a finding; the rest is redacted.
const SHOWN: usize = 4;

pub struct HardcodedSecret;

impl Rule for HardcodedSecret {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, asset: &Asset, _ctx: &RuleCtx<'_>, out: &mut Vec<Finding>) {
        let src = asset.source.as_str();
        if !MATCHERS.set.is_match(src) {
            return;
        }
        let mut hits = Vec::new();
        for i in &MATCHERS.set.matches(src) {
            let provider = &PROVIDERS[i];
            let Some(re) = MATCHERS.regex(i) else {
                continue;
            };
            for caps in re.captures_iter(src) {
                let Some(m) = caps.get(1).or_else(|| caps.get(0)) else {
                    continue;
                };
                let secret = m.as_str();
                // AI-SEC-003 FP guards: placeholders, env references, words and code.
                if is_placeholder(secret) || !plausible(provider.shape, secret) {
                    continue;
                }
                hits.push((m.start(), i, m.end()));
            }
        }
        // Overlapping matches: the earliest start wins, then the earlier provider.
        hits.sort_unstable();
        let mut covered = 0;
        for (start, i, end) in hits {
            if start < covered {
                continue;
            }
            covered = end;
            let (Ok(s), Ok(e)) = (u32::try_from(start), u32::try_from(end)) else {
                continue;
            };
            let message = format!(
                "Possible {} secret \"{}\"",
                PROVIDERS[i].name,
                redact(&src[start..end])
            );
            out.push(Finding::new(&META, asset, Span::new(s, e), message));
        }
    }
}

/// The first [`SHOWN`] chars of `secret` followed by `…`.
fn redact(secret: &str) -> String {
    let mut s: String = secret.chars().take(SHOWN).collect();
    s.push('…');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_keeps_four_chars() {
        assert_eq!(redact("abcdefgh"), "abcd…");
        assert_eq!(redact("ab"), "ab…");
    }
}
