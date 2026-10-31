//! Which assistant loads an always-on asset (ADR-042 load table, ADR-045).

use std::fmt;

use serde::Serialize;

use crate::asset::in_dir;
use crate::model::AssetKind;

/// A coding assistant with its own set of always-loaded files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Assistant {
    Copilot,
    ClaudeCode,
    Cursor,
    Windsurf,
    Cline,
    GeminiCli,
    Codex,
}

impl Assistant {
    /// Every assistant, in declaration order.
    pub const ALL: &'static [Assistant] = &[
        Assistant::Copilot,
        Assistant::ClaudeCode,
        Assistant::Cursor,
        Assistant::Windsurf,
        Assistant::Cline,
        Assistant::GeminiCli,
        Assistant::Codex,
    ];

    /// The display name (`Claude Code`).
    pub const fn name(self) -> &'static str {
        match self {
            Assistant::Copilot => "Copilot",
            Assistant::ClaudeCode => "Claude Code",
            Assistant::Cursor => "Cursor",
            Assistant::Windsurf => "Windsurf",
            Assistant::Cline => "Cline",
            Assistant::GeminiCli => "Gemini CLI",
            Assistant::Codex => "Codex",
        }
    }

    const fn bit(self) -> u8 {
        1 << self as u8
    }
}

impl fmt::Display for Assistant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

// `AssistantSet` is a `u8`, and `ALL` must list the variants in discriminant order.
const _: () = assert!(Assistant::ALL.len() <= 8);

/// A set of [`Assistant`]s, stored as a `u8` bitset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct AssistantSet(u8);

impl AssistantSet {
    const fn of(list: &[Assistant]) -> Self {
        let mut bits = 0;
        let mut i = 0;
        while i < list.len() {
            bits |= list[i].bit();
            i += 1;
        }
        Self(bits)
    }

    pub const fn contains(self, a: Assistant) -> bool {
        self.0 & a.bit() != 0
    }

    /// Assistants in the set, in declaration order.
    pub fn iter(self) -> impl Iterator<Item = Assistant> {
        Assistant::ALL
            .iter()
            .copied()
            .filter(move |a| self.contains(*a))
    }
}

/// The assistants that load the always-on part of the asset at `rel_path` (project level,
/// ADR-042). Skills and agents elsewhere (plugin folders) count for Copilot and Claude Code,
/// the only assistants that load them. `None` when no row names the location (tools, custom
/// assets): it then counts for every assistant.
pub fn loaded_by(rel_path: &str, kind: AssetKind) -> Option<AssistantSet> {
    use Assistant::*;
    let name = rel_path.rsplit('/').next().unwrap_or(rel_path);
    let list: &[Assistant] = match kind {
        AssetKind::Instructions => match name {
            "CLAUDE.md" => &[Copilot, ClaudeCode],
            "CLAUDE.local.md" => &[ClaudeCode],
            "AGENTS.md" => &[Copilot, Cursor, Windsurf, Codex],
            "GEMINI.md" => &[GeminiCli],
            "copilot-instructions.md" => &[Copilot],
            ".cursorrules" => &[Cursor],
            ".windsurfrules" => &[Windsurf],
            ".clinerules" => &[Cline],
            _ => return None,
        },
        AssetKind::ScopedInstructions => match () {
            _ if in_dir(rel_path, ".cursor/rules/") => &[Cursor],
            _ if in_dir(rel_path, ".claude/rules/") => &[Copilot, ClaudeCode],
            _ if in_dir(rel_path, ".github/instructions/") => &[Copilot],
            _ if in_dir(rel_path, ".windsurf/rules/") => &[Windsurf],
            _ if in_dir(rel_path, ".clinerules/") => &[Cline],
            _ => return None,
        },
        AssetKind::Skill => match () {
            _ if in_dir(rel_path, ".github/skills/") => &[Copilot],
            _ if in_dir(rel_path, ".claude/skills/") => &[Copilot, ClaudeCode],
            _ if in_dir(rel_path, ".agents/skills/") => &[Copilot, Codex],
            _ => &[Copilot, ClaudeCode],
        },
        AssetKind::Agent => match () {
            _ if in_dir(rel_path, ".claude/agents/") => &[Copilot, ClaudeCode],
            _ if in_dir(rel_path, ".github/agents/") || in_dir(rel_path, ".github/chatmodes/") => {
                &[Copilot]
            }
            _ => &[Copilot, ClaudeCode],
        },
        AssetKind::Prompt => match () {
            _ if in_dir(rel_path, ".claude/commands/") => &[ClaudeCode],
            _ if in_dir(rel_path, ".github/prompts/") => &[Copilot],
            _ => return None,
        },
        _ => return None,
    };
    Some(AssistantSet::of(list))
}

/// Always-on tokens per assistant from `(assistants, tokens)` items: an item with `None`
/// counts for every assistant. Only assistants named by some item with tokens are listed,
/// largest first (ties in declaration order). The second value is the largest total, or the
/// sum of all items when no assistant is named.
pub fn per_assistant(
    items: impl IntoIterator<Item = (Option<AssistantSet>, u64)>,
) -> (Vec<(Assistant, u64)>, u64) {
    let mut totals = [0u64; Assistant::ALL.len()];
    let mut named = AssistantSet::default();
    let mut any = 0u64;
    for (set, tokens) in items {
        if tokens == 0 {
            continue;
        }
        match set {
            Some(set) => {
                named.0 |= set.0;
                for a in set.iter() {
                    totals[a as usize] += tokens;
                }
            }
            None => any += tokens,
        }
    }
    let mut list: Vec<(Assistant, u64)> = named
        .iter()
        .map(|a| (a, totals[a as usize] + any))
        .collect();
    list.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let largest = list.first().map_or(any, |&(_, t)| t);
    (list, largest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use AssetKind::*;
    use Assistant::*;

    fn names(rel: &str, kind: AssetKind) -> Option<Vec<Assistant>> {
        loaded_by(rel, kind).map(|s| s.iter().collect())
    }

    #[test]
    fn load_table_rows() {
        for (rel, kind, want) in [
            ("CLAUDE.md", Instructions, Some(vec![Copilot, ClaudeCode])),
            (
                ".claude/CLAUDE.md",
                Instructions,
                Some(vec![Copilot, ClaudeCode]),
            ),
            ("CLAUDE.local.md", Instructions, Some(vec![ClaudeCode])),
            (
                "AGENTS.md",
                Instructions,
                Some(vec![Copilot, Cursor, Windsurf, Codex]),
            ),
            (
                ".github/copilot-instructions.md",
                Instructions,
                Some(vec![Copilot]),
            ),
            ("pkg/GEMINI.md", Instructions, Some(vec![GeminiCli])),
            (
                ".cursor/rules/a.mdc",
                ScopedInstructions,
                Some(vec![Cursor]),
            ),
            (
                ".claude/rules/a.md",
                ScopedInstructions,
                Some(vec![Copilot, ClaudeCode]),
            ),
            (
                ".github/instructions/a.instructions.md",
                ScopedInstructions,
                Some(vec![Copilot]),
            ),
            (
                ".claude/skills/x/SKILL.md",
                Skill,
                Some(vec![Copilot, ClaudeCode]),
            ),
            (
                ".agents/skills/x/SKILL.md",
                Skill,
                Some(vec![Copilot, Codex]),
            ),
            ("skills/x/SKILL.md", Skill, Some(vec![Copilot, ClaudeCode])),
            (
                ".github/chatmodes/a.chatmode.md",
                Agent,
                Some(vec![Copilot]),
            ),
            ("agent.tools.json", Tools, None),
            ("x.txt", Custom, None),
        ] {
            assert_eq!(names(rel, kind), want, "{rel}");
        }
    }

    #[test]
    fn per_assistant_adds_unnamed_items_to_every_assistant() {
        let claude = loaded_by("CLAUDE.local.md", Instructions);
        let agents = loaded_by("AGENTS.md", Instructions);
        let (list, largest) = per_assistant([(claude, 100), (agents, 30), (None, 5)]);
        assert_eq!(
            list,
            vec![
                (ClaudeCode, 105),
                (Copilot, 35),
                (Cursor, 35),
                (Windsurf, 35),
                (Codex, 35)
            ]
        );
        assert_eq!(largest, 105);
        assert_eq!(per_assistant([(None, 5), (claude, 0)]), (vec![], 5));
    }
}
