//! Text of the context-load block, shared by the terminal, Markdown and GitHub reports.

use pulse_core::Skip;
use pulse_engine::{ContextLoad, NotLoaded, Report, Source};

use crate::terminal::{plural, tokens};

/// What the CONTEXT LOAD block measures, beside its title.
pub(crate) const MEANING: &str = "the tokens each assistant reads on every request";

/// Explains the context-debt number wherever it is shown.
pub(crate) const DEBT_MEANING: &str =
    "share of tokens that is avoidable waste (duplicates, filler, over-budget excess)";

/// How many always-loaded entries are listed before `…`.
const TOP: usize = 4;

/// Width of a terminal bar, in cells (narrower terminals get down to half).
pub(crate) const BAR: usize = 20;

/// Always-on tokens against `budgets.always_on`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Level {
    /// At most 75% of the budget.
    Ok,
    /// Over 75% of the budget, up to the budget.
    Near,
    Over,
}

/// Where `t` always-on tokens stand against `budget`.
pub(crate) fn level(t: u64, budget: u64) -> Level {
    match t {
        t if t > budget => Level::Over,
        t if t.saturating_mul(4) > budget.saturating_mul(3) => Level::Near,
        _ => Level::Ok,
    }
}

/// One row per listed assistant, largest first; a single `Every assistant` row when no
/// always-on file names one.
pub(crate) fn rows(c: &ContextLoad) -> Vec<(&'static str, u64)> {
    if c.by_assistant.is_empty() {
        return vec![("Every assistant", c.always_on_tokens)];
    }
    c.by_assistant.iter().map(|&(a, t)| (a.name(), t)).collect()
}

/// `t` as a share of the window and of the budget.
pub(crate) fn shares(c: &ContextLoad, t: u64) -> (f64, f64) {
    (
        t as f64 / c.window.max(1) as f64,
        t as f64 / c.budget.max(1) as f64,
    )
}

/// `of 200k window`, `of 8k budget`: the column headers of the shares.
pub(crate) fn share_headers(c: &ContextLoad) -> (String, String) {
    (
        format!("of {} window", tokens(c.window)),
        format!("of {} budget", tokens(c.budget)),
    )
}

/// What the budget is: `8k always-on tokens per assistant (budgets.always_on in pulse.toml)`.
pub(crate) fn budget(c: &ContextLoad) -> String {
    format!(
        "{} always-on tokens per assistant (`budgets.always_on` in pulse.toml)",
        tokens(c.budget)
    )
}

/// Filled cells of a `bar`-cell bar: the share of the window, at least one when anything
/// loads.
pub(crate) fn filled(t: u64, window: u64, bar: usize) -> usize {
    let cells = (t as f64 / window.max(1) as f64 * bar as f64).round() as usize;
    if t > 0 { cells.clamp(1, bar) } else { 0 }
}

/// Whether the assistants load different totals.
fn differ(c: &ContextLoad) -> bool {
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
    let whose: Vec<&str> = if differ(c) {
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

/// `Copilot only (--assistant) · 476 assets scanned`, when the scan was restricted.
pub(crate) fn scope(report: &Report) -> Option<String> {
    let only = report.assistants?;
    let names: Vec<&str> = only.iter().map(|a| a.name()).collect();
    let assets = report.metrics.assets_by_kind.values().sum();
    Some(format!(
        "{} only (--assistant) · {} scanned",
        and_list(&names),
        plural(assets, "asset", "assets")
    ))
}

/// What `--assistant` left out and why: `12 files Copilot doesn't load were not scanned:
/// 9 library copies (no assistant loads them from where they are) · 3 for Cursor`.
pub(crate) fn not_loaded(report: &Report) -> Option<String> {
    let only = report.assistants?;
    let names: Vec<&str> = only.iter().map(|a| a.name()).collect();
    let (who, loads) = match names.as_slice() {
        [one] => ((*one).to_owned(), "doesn't load"),
        [a, b] => (format!("neither {a} nor {b}"), "loads"),
        _ => (format!("none of {}", and_list(&names)), "loads"),
    };
    if report.not_loaded.is_empty() {
        return Some(format!(
            "nothing: every AI file found is loaded by {}",
            and_list(&names)
        ));
    }
    let mut groups: Vec<(String, usize)> = Vec::new();
    for n in &report.not_loaded {
        let label = match n.reason {
            Skip::Library => {
                "library copies (no assistant loads them from where they are)".to_owned()
            }
            Skip::OtherAssistants(_) => format!("for {}", owners(n)),
        };
        match groups.iter_mut().find(|(l, _)| *l == label) {
            Some(g) => g.1 += 1,
            None => groups.push((label, 1)),
        }
    }
    groups.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let parts: Vec<String> = groups.iter().map(|(l, n)| format!("{n} {l}")).collect();
    let files = report.not_loaded.len();
    let verb = if files == 1 { "was" } else { "were" };
    Some(format!(
        "{} {who} {loads} {verb} not scanned: {}",
        plural(files, "file", "files"),
        parts.join(" · ")
    ))
}

/// Why one file was left out: `library copy` or `for Cursor, Windsurf`.
pub(crate) fn not_loaded_reason(n: &NotLoaded) -> String {
    match n.reason {
        Skip::Library => "library copy: no assistant loads it from where it is".to_owned(),
        Skip::OtherAssistants(_) => format!("for {}", owners(n)),
    }
}

fn owners(n: &NotLoaded) -> String {
    match n.reason {
        Skip::OtherAssistants(set) => and_list(&set.iter().map(|a| a.name()).collect::<Vec<_>>()),
        Skip::Library => String::new(),
    }
}

/// `A`, `A and B`, `A, B and C`.
fn and_list(names: &[&str]) -> String {
    match names {
        [] => String::new(),
        [one] => (*one).to_owned(),
        [init @ .., last] => format!("{} and {last}", init.join(", ")),
    }
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
        assert_eq!(level(6000, 8000), Level::Ok);
        assert_eq!(level(6001, 8000), Level::Near);
        assert_eq!(level(8000, 8000), Level::Near);
        assert_eq!(level(8001, 8000), Level::Over);
    }

    #[test]
    fn bar_cells_follow_the_window_share() {
        assert_eq!(filled(0, 200_000, BAR), 0);
        assert_eq!(filled(10, 200_000, BAR), 1);
        assert_eq!(filled(42_700, 200_000, BAR), 4);
        assert_eq!(filled(900_000, 200_000, BAR), BAR);
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
        assert!(!differ(&c));
        assert!(always_loaded(&c, str::to_owned).ends_with("(Copilot, Claude Code)"));
    }

    #[test]
    fn rows_fall_back_to_every_assistant() {
        let mut c = load(70, 8000);
        assert_eq!(rows(&c), vec![("Every assistant", 70)]);
        c.by_assistant = vec![(Assistant::Copilot, 70), (Assistant::ClaudeCode, 66)];
        assert_eq!(rows(&c), vec![("Copilot", 70), ("Claude Code", 66)]);
        assert_eq!(shares(&c, 2000), (0.01, 0.25));
    }

    fn scoped(only: &[Assistant], left_out: Vec<(&str, Skip)>) -> Report {
        Report {
            assistants: Some(only.iter().copied().collect()),
            not_loaded: left_out
                .into_iter()
                .map(|(p, reason)| NotLoaded {
                    rel_path: p.to_owned(),
                    kind: pulse_core::AssetKind::Instructions,
                    reason,
                })
                .collect(),
            ..Report::default()
        }
    }

    #[test]
    fn not_loaded_groups_reasons_and_names_the_selection() {
        use Assistant::*;
        let cursor = Skip::OtherAssistants([Cursor].into_iter().collect());
        let both = Skip::OtherAssistants([Cursor, Windsurf].into_iter().collect());
        assert!(not_loaded(&Report::default()).is_none());
        assert_eq!(
            not_loaded(&scoped(&[Copilot], vec![])).unwrap(),
            "nothing: every AI file found is loaded by Copilot"
        );
        assert_eq!(
            not_loaded(&scoped(&[Copilot], vec![("a", cursor)])).unwrap(),
            "1 file Copilot doesn't load was not scanned: 1 for Cursor"
        );
        let r = scoped(
            &[Copilot, ClaudeCode],
            vec![("a", Skip::Library), ("b", both), ("c", Skip::Library)],
        );
        assert_eq!(
            not_loaded(&r).unwrap(),
            "3 files neither Copilot nor Claude Code loads were not scanned: 2 library copies \
             (no assistant loads them from where they are) · 1 for Cursor and Windsurf"
        );
        assert_eq!(
            not_loaded_reason(&r.not_loaded[1]),
            "for Cursor and Windsurf"
        );
        let three = scoped(&[Copilot, Cursor, Codex], vec![("a", Skip::Library)]);
        assert!(
            not_loaded(&three)
                .unwrap()
                .starts_with("1 file none of Copilot, Cursor and Codex loads was")
        );
        assert_eq!(
            scope(&three).unwrap(),
            "Copilot, Cursor and Codex only (--assistant) · 0 assets scanned"
        );
    }
}
