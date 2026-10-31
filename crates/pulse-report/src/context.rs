//! Text of the context-load block, shared by the terminal, Markdown and GitHub reports.

use pulse_engine::{ContextLoad, Source};

use crate::terminal::{pct, tokens};

/// Explains the context-debt number wherever it is shown.
pub(crate) const DEBT_MEANING: &str =
    "share of tokens that is avoidable waste (duplicates, filler, over-budget excess)";

/// How many always-loaded entries are listed before `…`.
const TOP: usize = 4;

/// Width of the terminal bar, in cells.
pub(crate) const BAR: usize = 30;

/// Always-on tokens against `budgets.always_on`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Level {
    /// At most 75% of the budget.
    Ok,
    /// Over 75% of the budget, up to the budget.
    Near,
    Over,
}

pub(crate) fn level(c: &ContextLoad) -> Level {
    match c.always_on_tokens {
        t if t > c.budget => Level::Over,
        t if t.saturating_mul(4) > c.budget.saturating_mul(3) => Level::Near,
        _ => Level::Ok,
    }
}

/// `42.7k tokens on every request · 21.3% of a 200k window`.
pub(crate) fn headline(c: &ContextLoad) -> String {
    format!(
        "{} tokens on every request · {} of a {} window",
        tokens(c.always_on_tokens),
        pct(c.ratio()),
        tokens(c.window)
    )
}

/// Filled cells of the bar: the share of the window, at least one when anything loads.
pub(crate) fn filled(c: &ContextLoad) -> usize {
    let cells = (c.ratio() * BAR as f64).round() as usize;
    if c.always_on_tokens > 0 {
        cells.clamp(1, BAR)
    } else {
        0
    }
}

/// Whether the assistants load different totals, so each gets its own line.
pub(crate) fn per_assistant_lines(c: &ContextLoad) -> bool {
    c.by_assistant.windows(2).any(|w| w[0].1 != w[1].1)
}

/// `AGENTS.md 12.1k · 465 skill descriptions 18k · … (Copilot)`, each file name wrapped by
/// `name`; ends with the largest assistant, or every assistant when they all load the same.
pub(crate) fn always_loaded(c: &ContextLoad, name: impl Fn(&str) -> String) -> String {
    if c.contributors.is_empty() {
        return "nothing: no always-on instructions, skills or agents".to_owned();
    }
    let mut parts: Vec<String> = c
        .contributors
        .iter()
        .take(TOP)
        .map(|e| {
            let label = match &e.source {
                Source::File(path) => name(path),
                Source::SkillDescriptions(n) => descriptions(*n, "skill"),
                Source::AgentDescriptions(n) => descriptions(*n, "agent"),
            };
            format!("{label} {}", tokens(e.tokens))
        })
        .collect();
    if c.contributors.len() > TOP {
        parts.push("…".to_owned());
    }
    let list = parts.join(" · ");
    let whose: Vec<&str> = if per_assistant_lines(c) {
        c.by_assistant
            .iter()
            .take(1)
            .map(|(a, _)| a.name())
            .collect()
    } else {
        c.by_assistant.iter().map(|(a, _)| a.name()).collect()
    };
    if whose.is_empty() {
        list
    } else {
        format!("{list} ({})", whose.join(", "))
    }
}

fn descriptions(n: usize, what: &str) -> String {
    let s = if n == 1 { "" } else { "s" };
    format!("{n} {what} description{s}")
}

/// `1.0M tokens in skills, agents, prompts and scoped rules, loaded only when used`.
pub(crate) fn on_demand(c: &ContextLoad) -> String {
    format!(
        "{} tokens in skills, agents, prompts and scoped rules, loaded only when used",
        tokens(c.on_demand_tokens)
    )
}

#[cfg(test)]
mod tests {
    use pulse_core::Assistant;
    use pulse_engine::Contributor;

    use super::*;

    fn load(always: u64, budget: u64) -> ContextLoad {
        ContextLoad {
            always_on_tokens: always,
            window: 200_000,
            budget,
            ..ContextLoad::default()
        }
    }

    #[test]
    fn level_against_budget() {
        assert_eq!(level(&load(6000, 8000)), Level::Ok);
        assert_eq!(level(&load(6001, 8000)), Level::Near);
        assert_eq!(level(&load(8000, 8000)), Level::Near);
        assert_eq!(level(&load(8001, 8000)), Level::Over);
    }

    #[test]
    fn bar_cells_follow_the_window_share() {
        assert_eq!(filled(&load(0, 8000)), 0);
        assert_eq!(filled(&load(10, 8000)), 1);
        assert_eq!(filled(&load(42_700, 8000)), 6);
        assert_eq!(filled(&load(900_000, 8000)), BAR);
    }

    #[test]
    fn always_loaded_lists_top_entries_and_names_the_assistant() {
        let file = |p: &str, t| Contributor {
            source: Source::File(p.to_owned()),
            tokens: t,
        };
        let mut c = load(0, 8000);
        assert!(always_loaded(&c, str::to_owned).starts_with("nothing"));
        c.contributors = vec![
            file("AGENTS.md", 12_100),
            Contributor {
                source: Source::SkillDescriptions(465),
                tokens: 18_000,
            },
            file("a", 3),
            file("b", 2),
            file("c", 1),
        ];
        c.by_assistant = vec![(Assistant::Copilot, 2), (Assistant::Cursor, 1)];
        assert_eq!(
            always_loaded(&c, |p| format!("`{p}`")),
            "`AGENTS.md` 12.1k · 465 skill descriptions 18k · `a` 3 · `b` 2 · … (Copilot)"
        );
        c.by_assistant = vec![(Assistant::Copilot, 1), (Assistant::ClaudeCode, 1)];
        assert!(!per_assistant_lines(&c));
        assert!(always_loaded(&c, str::to_owned).ends_with("(Copilot, Claude Code)"));
    }
}
