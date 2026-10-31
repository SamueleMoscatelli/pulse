//! Which assistant loads an always-on asset (ADR-042 load table, ADR-045).

use std::fmt;

use serde::Serialize;

use crate::asset::in_dir;
use crate::model::{AssetKind, Origin};

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

    /// The CLI and config spelling (`claude-code`).
    pub const fn id(self) -> &'static str {
        match self {
            Assistant::Copilot => "copilot",
            Assistant::ClaudeCode => "claude-code",
            Assistant::Cursor => "cursor",
            Assistant::Windsurf => "windsurf",
            Assistant::Cline => "cline",
            Assistant::GeminiCli => "gemini-cli",
            Assistant::Codex => "codex",
        }
    }

    /// The assistant spelled `id` (case-insensitive).
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|a| a.id().eq_ignore_ascii_case(id))
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

    /// Every assistant.
    pub const ALL: Self = Self::of(Assistant::ALL);

    pub const fn contains(self, a: Assistant) -> bool {
        self.0 & a.bit() != 0
    }

    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
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
        AssetKind::McpConfig => match () {
            _ if in_dir(rel_path, ".vscode/") => &[Copilot],
            _ if in_dir(rel_path, ".cursor/") => &[Cursor],
            _ if in_dir(rel_path, ".gemini/") => &[GeminiCli],
            _ if name == ".mcp.json" => &[ClaudeCode],
            // `claude_desktop_config.json` belongs to no coding assistant.
            _ => return None,
        },
        AssetKind::Settings => &[ClaudeCode],
        AssetKind::Hooks => match () {
            _ if in_dir(rel_path, ".cursor/") => &[Cursor],
            _ if in_dir(rel_path, ".github/hooks/") => &[Copilot],
            _ if in_dir(rel_path, ".claude/") => &[ClaudeCode],
            // Another assistant's folder: no row, so it stays in every scan.
            _ if rel_path.split('/').any(|seg| seg.starts_with('.')) => return None,
            // `hooks/hooks.json` in a plugin.
            _ => &[Copilot, ClaudeCode],
        },
        _ => return None,
    };
    Some(AssistantSet::of(list))
}

impl FromIterator<Assistant> for AssistantSet {
    fn from_iter<I: IntoIterator<Item = Assistant>>(iter: I) -> Self {
        Self(iter.into_iter().fold(0, |bits, a| bits | a.bit()))
    }
}

/// Why a file is left out of a scan for some assistants (`--assistant`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skip {
    /// Loaded from where it is, but only by these other assistants.
    OtherAssistants(AssistantSet),
    /// A library copy (ADR-041): no assistant loads it from where it is.
    Library,
}

/// Why the file at `rel_path` is not loaded by any assistant in `only`; `None` keeps it,
/// also when no row names its location (tools, custom assets).
pub fn skip_for(
    rel_path: &str,
    kind: AssetKind,
    origin: Origin,
    only: AssistantSet,
) -> Option<Skip> {
    match origin {
        Origin::Custom => None,
        Origin::Library | Origin::LibraryRow => Some(Skip::Library),
        Origin::Platform => loaded_by(rel_path, kind)
            .filter(|set| !set.intersects(only))
            .map(Skip::OtherAssistants),
    }
}

/// Always-on tokens per assistant from `(assistants, tokens)` items: an item with `None`
/// counts for every assistant. Listed are the assistants in `only` (even with no tokens)
/// or, without it, those named by some item with tokens; largest first (ties in declaration
/// order). The second value is the largest total, or the sum of all items when none is
/// listed.
pub fn per_assistant(
    items: impl IntoIterator<Item = (Option<AssistantSet>, u64)>,
    only: Option<AssistantSet>,
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
    let mut list: Vec<(Assistant, u64)> = only
        .unwrap_or(named)
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
            (".vscode/mcp.json", McpConfig, Some(vec![Copilot])),
            (".mcp.json", McpConfig, Some(vec![ClaudeCode])),
            (".cursor/mcp.json", McpConfig, Some(vec![Cursor])),
            (".gemini/settings.json", McpConfig, Some(vec![GeminiCli])),
            ("claude_desktop_config.json", McpConfig, None),
            (".claude/settings.json", Settings, Some(vec![ClaudeCode])),
            (".cursor/hooks.json", Hooks, Some(vec![Cursor])),
            (".github/hooks/hooks.json", Hooks, Some(vec![Copilot])),
            (
                "plugin/hooks/hooks.json",
                Hooks,
                Some(vec![Copilot, ClaudeCode]),
            ),
            (".claude/hooks/hooks.json", Hooks, Some(vec![ClaudeCode])),
            (".gemini/hooks/hooks.json", Hooks, None),
            ("agent.tools.json", Tools, None),
            ("x.txt", Custom, None),
        ] {
            assert_eq!(names(rel, kind), want, "{rel}");
        }
    }

    #[test]
    fn skip_for_keeps_loaded_unnamed_and_custom_files() {
        let only: AssistantSet = [Copilot].into_iter().collect();
        let skip = |rel, kind, origin| skip_for(rel, kind, origin, only);
        assert_eq!(skip("CLAUDE.md", Instructions, Origin::Platform), None);
        assert_eq!(skip("agent.tools.json", Tools, Origin::Platform), None);
        assert_eq!(skip("rules/x.md", Custom, Origin::Custom), None);
        assert_eq!(
            skip(".cursor/rules/a.mdc", ScopedInstructions, Origin::Platform),
            Some(Skip::OtherAssistants([Cursor].into_iter().collect()))
        );
        assert_eq!(
            skip("agents/a.agent.md", Agent, Origin::LibraryRow),
            Some(Skip::Library)
        );
        assert_eq!(
            skip(".github/agents/a.agent.md", Agent, Origin::Library),
            Some(Skip::Library)
        );
    }

    #[test]
    fn assistant_ids_round_trip() {
        for &a in Assistant::ALL {
            assert_eq!(Assistant::from_id(a.id()), Some(a));
        }
        assert_eq!(Assistant::from_id("Claude-Code"), Some(ClaudeCode));
        assert_eq!(Assistant::from_id("claude"), None);
    }

    #[test]
    fn per_assistant_adds_unnamed_items_to_every_assistant() {
        let claude = loaded_by("CLAUDE.local.md", Instructions);
        let agents = loaded_by("AGENTS.md", Instructions);
        let (list, largest) = per_assistant([(claude, 100), (agents, 30), (None, 5)], None);
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
        assert_eq!(per_assistant([(None, 5), (claude, 0)], None), (vec![], 5));
    }

    #[test]
    fn per_assistant_lists_the_selection_even_without_tokens() {
        let agents = loaded_by("AGENTS.md", Instructions);
        let only = [GeminiCli, Copilot].into_iter().collect();
        assert_eq!(
            per_assistant([(agents, 30), (None, 5)], Some(only)),
            (vec![(Copilot, 35), (GeminiCli, 5)], 35)
        );
    }
}
