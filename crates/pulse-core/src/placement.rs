//! Near-miss names and folders: files that look like assets but sit where no assistant
//! loads them (spec 03-rules → AI-STR-008, ADR-042).

use crate::asset::in_dir;

/// Why a file is not loaded, and what it should be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NearMiss {
    /// Wrong letter case: loads only on case-insensitive file systems. Holds the right name.
    Case(&'static str),
    /// Misspelled file name. Holds the right name.
    Rename(&'static str),
    /// Right name, wrong folder. Holds the expected location.
    Move(&'static str),
}

/// A walked file that is not an asset only because of its name or folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Misplaced {
    /// Path relative to the scan root, always with `/` separators.
    pub rel_path: String,
    pub near_miss: NearMiss,
}

/// Misspelled platform folders, and the folder assistants load instead.
const FOLDERS: &[(&str, &str)] = &[
    (".github/agent/", ".github/agents/"),
    (".github/instruction/", ".github/instructions/"),
    (".github/prompt/", ".github/prompts/"),
    (".claude/agent/", ".claude/agents/"),
    (".claude/command/", ".claude/commands/"),
    (".claude/skill/", ".claude/skills/"),
    (".kiro/steerings/", ".kiro/steering/"),
    (".kiro/skill/", ".kiro/skills/"),
    (".kiro/hook/", ".kiro/hooks/"),
    (".kiro/agent/", ".kiro/agents/"),
];

/// Kiro folders that also hold non-Markdown assets (`*.kiro.hook`, JSON agents and hooks).
const KIRO_DATA_FOLDERS: &[(&str, &str)] = &[
    (".kiro/hook/", ".kiro/hooks/"),
    (".kiro/agent/", ".kiro/agents/"),
];

/// The near miss `rel_path` is, by name and folder only. Files that are not `.md`/`.mdc`/
/// `.json`/`.hook` return after a suffix test, so this is cheap enough to run on every
/// walked file.
pub fn near_miss(rel_path: &str) -> Option<NearMiss> {
    let (dir, name) = rel_path.rsplit_once('/').unwrap_or(("", rel_path));
    if name.ends_with(".mdc") {
        return (!in_dir(rel_path, ".cursor/rules/")).then_some(NearMiss::Move(".cursor/rules/"));
    }
    if name.ends_with(".json") || name.ends_with(".hook") {
        return KIRO_DATA_FOLDERS
            .iter()
            .find(|(wrong, _)| in_dir(rel_path, wrong))
            .map(|&(_, right)| NearMiss::Move(right));
    }
    if !name.ends_with(".md") {
        return None;
    }
    if name.eq_ignore_ascii_case("SKILL.md") && name != "SKILL.md" {
        return Some(NearMiss::Rename("SKILL.md"));
    }
    if name.eq_ignore_ascii_case("CLAUDE.md") && name != "CLAUDE.md" {
        return Some(NearMiss::Case("CLAUDE.md"));
    }
    if name == "copilot-instruction.md" {
        return Some(NearMiss::Rename("copilot-instructions.md"));
    }
    if name == "copilot-instructions.md" && dir != ".github" && !dir.ends_with("/.github") {
        return Some(NearMiss::Move(".github/"));
    }
    FOLDERS
        .iter()
        .find(|(wrong, _)| in_dir(rel_path, wrong))
        .map(|&(_, right)| NearMiss::Move(right))
}

#[cfg(test)]
mod tests {
    use super::*;
    use NearMiss::*;

    #[test]
    fn near_miss_names_and_folders() {
        for (rel, want) in [
            ("a/skill.md", Some(Rename("SKILL.md"))),
            (".claude/skills/x/Skill.md", Some(Rename("SKILL.md"))),
            ("claude.md", Some(Case("CLAUDE.md"))),
            ("pkg/Claude.md", Some(Case("CLAUDE.md"))),
            (
                ".github/copilot-instruction.md",
                Some(Rename("copilot-instructions.md")),
            ),
            ("copilot-instructions.md", Some(Move(".github/"))),
            (
                "docs/.github/instructions/copilot-instructions.md",
                Some(Move(".github/")),
            ),
            ("rules/style.mdc", Some(Move(".cursor/rules/"))),
            (".cursor/style.mdc", Some(Move(".cursor/rules/"))),
            (".github/agent/a.md", Some(Move(".github/agents/"))),
            ("pkg/.claude/command/x.md", Some(Move(".claude/commands/"))),
            (".claude/skill/x/README.md", Some(Move(".claude/skills/"))),
            (".kiro/steerings/style.md", Some(Move(".kiro/steering/"))),
            (".kiro/skill/x/README.md", Some(Move(".kiro/skills/"))),
            (".kiro/hook/fmt.kiro.hook", Some(Move(".kiro/hooks/"))),
            (".kiro/hook/fmt.json", Some(Move(".kiro/hooks/"))),
            (".kiro/agent/a.json", Some(Move(".kiro/agents/"))),
            ("pkg/.kiro/agent/a.md", Some(Move(".kiro/agents/"))),
            // Correct names and places, and unrelated files.
            ("SKILL.md", None),
            ("CLAUDE.md", None),
            (".github/copilot-instructions.md", None),
            ("pkg/.github/copilot-instructions.md", None),
            (".cursor/rules/a/style.mdc", None),
            (".claude/agents/a.md", None),
            (".kiro/steering/style.md", None),
            (".kiro/hooks/fmt.kiro.hook", None),
            (".kiro/agents/a.json", None),
            (".kiro/steerings/data.json", None),
            ("package.json", None),
            ("x.claude/agent/a.md", None),
            (".github/agent/a.txt", None),
            ("src/main.rs", None),
            ("README.md", None),
        ] {
            assert_eq!(near_miss(rel), want, "{rel}");
        }
    }
}
