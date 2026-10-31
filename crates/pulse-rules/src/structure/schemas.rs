//! Frontmatter schemas per (kind, platform): required and known top-level keys.

use pulse_core::{Asset, AssetKind};

/// Which keys a frontmatter must have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Required {
    None,
    /// Every key.
    All(&'static [&'static str]),
    /// At least one of the keys.
    AnyOf(&'static [&'static str]),
}

/// The frontmatter contract of one asset flavor.
#[derive(Debug)]
pub struct Schema {
    /// Shown in messages, e.g. "Claude skill".
    pub label: &'static str,
    pub required: Required,
    /// Every recognized key, required ones included.
    pub known: &'static [&'static str],
}

impl Schema {
    pub fn is_known(&self, key: &str) -> bool {
        self.known.contains(&key)
    }
}

/// Keys of Claude skills and commands (they share one frontmatter format).
const CLAUDE_SKILL_KEYS: &[&str] = &[
    "name",
    "description",
    "license",
    "compatibility",
    "metadata",
    "version",
    "allowed-tools",
    "argument-hint",
    "disable-model-invocation",
    "user-invocable",
    "when_to_use",
    "model",
    "effort",
    "context",
    "agent",
    "hooks",
];

pub static CLAUDE_SKILL: Schema = Schema {
    label: "Claude skill",
    required: Required::All(&["name", "description"]),
    known: CLAUDE_SKILL_KEYS,
};

pub static CLAUDE_AGENT: Schema = Schema {
    label: "Claude agent",
    required: Required::All(&["name", "description"]),
    known: &[
        "name",
        "description",
        "tools",
        "disallowedTools",
        "model",
        "permissionMode",
        "skills",
        "hooks",
        "mcpServers",
        "memory",
        "maxTurns",
        "color",
        "effort",
        "background",
    ],
};

pub static CLAUDE_COMMAND: Schema = Schema {
    label: "Claude command",
    required: Required::None,
    known: CLAUDE_SKILL_KEYS,
};

pub static CURSOR_RULE: Schema = Schema {
    label: "Cursor rule",
    required: Required::AnyOf(&["description", "globs", "alwaysApply"]),
    known: &["description", "globs", "alwaysApply"],
};

pub static COPILOT_INSTRUCTIONS: Schema = Schema {
    label: "Copilot instructions",
    required: Required::All(&["applyTo"]),
    known: &["applyTo", "description", "name", "excludeAgent"],
};

pub static COPILOT_PROMPT: Schema = Schema {
    label: "Copilot prompt",
    required: Required::None,
    known: &[
        "name",
        "description",
        "argument-hint",
        "agent",
        "mode",
        "model",
        "tools",
    ],
};

pub static COPILOT_AGENT: Schema = Schema {
    label: "Copilot agent",
    required: Required::None,
    known: &[
        "name",
        "description",
        "argument-hint",
        "advancedOptions",
        "tools",
        "agents",
        "model",
        "model-policy",
        "reasoning-effort",
        "skills",
        "deferred-tool-loading",
        "strict-tools-list",
        "include-custom-instructions",
        "github",
        "user-invocable",
        "disable-model-invocation",
        "target",
        "mcp-servers",
        "handoffs",
        "hooks",
        "metadata",
        "infer",
    ],
};

/// Kiro steering (https://kiro.dev/docs/steering/); `inclusion` decides the required keys,
/// checked by AI-STR-001 itself. `title` is not documented but common in real steering (ADR-050).
pub static KIRO_STEERING: Schema = Schema {
    label: "Kiro steering",
    required: Required::None,
    known: &[
        "inclusion",
        "fileMatchPattern",
        "name",
        "description",
        "title",
    ],
};

/// The schema of `asset`, from its kind and path. `None` for flavors without a known
/// schema (`.prompty`, `.claude/rules`, Windsurf, Cline, custom kinds).
pub fn schema_for(asset: &Asset) -> Option<&'static Schema> {
    let path = asset.rel_path.as_str();
    match asset.kind {
        AssetKind::Skill => Some(&CLAUDE_SKILL),
        AssetKind::Agent if path.ends_with(".agent.md") || path.ends_with(".chatmode.md") => {
            Some(&COPILOT_AGENT)
        }
        AssetKind::Agent => Some(&CLAUDE_AGENT),
        AssetKind::Prompt if path.ends_with(".prompt.md") => Some(&COPILOT_PROMPT),
        AssetKind::Prompt if path.ends_with(".md") => Some(&CLAUDE_COMMAND),
        AssetKind::ScopedInstructions if asset.is_kiro_steering() => Some(&KIRO_STEERING),
        AssetKind::ScopedInstructions if path.ends_with(".mdc") => Some(&CURSOR_RULE),
        AssetKind::ScopedInstructions if path.ends_with(".instructions.md") => {
            Some(&COPILOT_INSTRUCTIONS)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn label(rel: &str, kind: AssetKind) -> Option<&'static str> {
        let a = Asset::new(
            PathBuf::from(rel),
            rel,
            kind,
            kind.default_load(),
            String::new(),
        );
        schema_for(&a).map(|s| s.label)
    }

    #[test]
    fn schema_from_kind_and_path() {
        use AssetKind::*;
        assert_eq!(
            label(".claude/skills/x/SKILL.md", Skill),
            Some("Claude skill")
        );
        assert_eq!(label(".claude/agents/a.md", Agent), Some("Claude agent"));
        assert_eq!(
            label(".github/agents/a.agent.md", Agent),
            Some("Copilot agent")
        );
        assert_eq!(
            label(".github/chatmodes/c.chatmode.md", Agent),
            Some("Copilot agent")
        );
        assert_eq!(
            label(".claude/commands/c.md", Prompt),
            Some("Claude command")
        );
        assert_eq!(
            label(".github/prompts/p.prompt.md", Prompt),
            Some("Copilot prompt")
        );
        assert_eq!(label("x.prompty", Prompt), None);
        assert_eq!(
            label(".cursor/rules/r.mdc", ScopedInstructions),
            Some("Cursor rule")
        );
        assert_eq!(
            label(".github/instructions/i.instructions.md", ScopedInstructions),
            Some("Copilot instructions")
        );
        assert_eq!(label(".claude/rules/r.md", ScopedInstructions), None);
        assert_eq!(
            label(".kiro/steering/r.md", ScopedInstructions),
            Some("Kiro steering")
        );
        assert_eq!(label(".windsurf/rules/r.md", ScopedInstructions), None);
    }

    #[test]
    fn known_keys_include_required() {
        for s in [
            &CLAUDE_SKILL,
            &CLAUDE_AGENT,
            &CLAUDE_COMMAND,
            &CURSOR_RULE,
            &COPILOT_INSTRUCTIONS,
            &COPILOT_PROMPT,
            &COPILOT_AGENT,
            &KIRO_STEERING,
        ] {
            if let Required::All(keys) | Required::AnyOf(keys) = s.required {
                assert!(keys.iter().all(|k| s.is_known(k)), "{}", s.label);
            }
        }
    }
}
