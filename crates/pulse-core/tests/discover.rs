//! Discovery over the `tests/fixtures/repos/mixed` tree.

use std::path::{Path, PathBuf};

use pulse_core::{AssetKind, Config, LoadMode, discover};

fn mixed() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/repos/mixed")
}

#[test]
fn discover_mixed_lists_exact_assets() -> Result<(), Box<dyn std::error::Error>> {
    use AssetKind::*;
    use LoadMode::*;
    let root = mixed();
    let config = Config::load(&root, None)?;
    let found = discover(&root, &config)?;
    let got: Vec<(&str, AssetKind, LoadMode)> = found
        .assets
        .iter()
        .map(|a| (a.rel_path.as_str(), a.kind, a.load))
        .collect();
    let expected = [
        (".claude/agents/planner.md", Agent, Always),
        (".claude/commands/deploy.md", Prompt, OnDemand),
        (".claude/rules/testing.md", ScopedInstructions, Conditional),
        (".claude/settings.json", Settings, Always),
        (".claude/skills/review/SKILL.md", Skill, Always),
        (".clinerules", Instructions, Always),
        (".cursor/hooks.json", Hooks, Always),
        (".cursor/mcp.json", McpConfig, Always),
        (".cursor/rules/style.mdc", ScopedInstructions, Conditional),
        (".cursorrules", Instructions, Always),
        (".gemini/settings.json", McpConfig, Always),
        (".github/agents/triage.agent.md", Agent, Always),
        (".github/chatmodes/debug.chatmode.md", Agent, Always),
        (".github/copilot-instructions.md", Instructions, Always),
        (
            ".github/instructions/rust.instructions.md",
            ScopedInstructions,
            Conditional,
        ),
        (".github/prompts/explain.prompt.md", Prompt, OnDemand),
        (".mcp.json", McpConfig, Always),
        (".vscode/mcp.json", McpConfig, Always),
        (".windsurf/rules/docs.md", ScopedInstructions, Conditional),
        (".windsurfrules", Instructions, Always),
        ("AGENTS.md", Instructions, Always),
        ("CLAUDE.local.md", Instructions, Always),
        ("CLAUDE.md", Instructions, Always),
        ("GEMINI.md", Instructions, Always),
        ("agent.tools.json", Tools, Always),
        ("claude_desktop_config.json", McpConfig, Always),
        ("packages/api/AGENTS.md", Instructions, Conditional),
        ("packages/api/CLAUDE.md", Instructions, Conditional),
        ("playbooks/handoff.txt", Prompt, OnDemand),
        ("plugin/hooks/hooks.json", Hooks, Always),
        ("prompts/summarize.prompty", Prompt, OnDemand),
        ("sub/.clinerules/naming.md", ScopedInstructions, Conditional),
        ("tools/fetch.json", Tools, Always),
    ];
    assert_eq!(got, expected);
    assert!(found.skipped.is_empty(), "{:?}", found.skipped);
    Ok(())
}

#[test]
fn discover_mixed_all_paths_honors_excludes() -> Result<(), Box<dyn std::error::Error>> {
    let root = mixed();
    let found = discover(&root, &Config::load(&root, None)?)?;
    for present in [
        "README.md",
        "tools/package.json",
        "pulse.toml",
        ".gitignore",
    ] {
        assert!(found.all_paths.contains(present), "missing {present}");
    }
    assert!(
        found
            .all_paths
            .iter()
            .all(|p| !p.starts_with("node_modules/"))
    );
    assert!(!found.all_paths.contains("ignored/CLAUDE.md"));
    Ok(())
}

#[test]
fn discover_scan_exclude_prunes() -> Result<(), Box<dyn std::error::Error>> {
    let root = mixed();
    let mut config = Config::load(&root, None)?;
    config.scan.exclude.push(".github".to_owned());
    config.scan.exclude.push("packages/**".to_owned());
    let found = discover(&root, &config)?;
    assert!(
        found
            .assets
            .iter()
            .all(|a| !a.rel_path.starts_with(".github/") && !a.rel_path.starts_with("packages/"))
    );
    Ok(())
}

#[test]
fn discover_pulseignore_is_honored() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    std::fs::write(dir.path().join("CLAUDE.md"), "x")?;
    std::fs::write(dir.path().join("AGENTS.md"), "x")?;
    std::fs::write(dir.path().join(".pulseignore"), "AGENTS.md\n")?;
    let found = discover(dir.path(), &Config::default())?;
    let rels: Vec<&str> = found.assets.iter().map(|a| a.rel_path.as_str()).collect();
    assert_eq!(rels, ["CLAUDE.md"]);
    Ok(())
}

#[test]
fn discover_rejects_file_root() {
    let file = mixed().join("CLAUDE.md");
    assert!(discover(&file, &Config::default()).is_err());
}

#[test]
fn discover_settings_local_and_include() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    std::fs::create_dir_all(dir.path().join(".claude"))?;
    std::fs::create_dir_all(dir.path().join("docs/ai"))?;
    std::fs::write(dir.path().join(".claude/settings.local.json"), "{}")?;
    std::fs::write(dir.path().join("docs/ai/notes.md"), "x")?;
    let mut config = Config::default();
    config.scan.include.push("docs/ai/*.md".to_owned());
    let found = discover(dir.path(), &config)?;
    let got: Vec<(&str, AssetKind, LoadMode)> = found
        .assets
        .iter()
        .map(|a| (a.rel_path.as_str(), a.kind, a.load))
        .collect();
    assert_eq!(
        got,
        [
            (
                ".claude/settings.local.json",
                AssetKind::Settings,
                LoadMode::Always
            ),
            ("docs/ai/notes.md", AssetKind::Custom, LoadMode::OnDemand),
        ]
    );
    Ok(())
}

#[test]
fn discover_ignores_gitignore_above_root() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    std::fs::write(dir.path().join(".gitignore"), "CLAUDE.md\n")?;
    let root = dir.path().join("repo");
    std::fs::create_dir_all(&root)?;
    std::fs::write(root.join("CLAUDE.md"), "x")?;
    let found = discover(&root, &Config::default())?;
    assert_eq!(found.assets.len(), 1);
    Ok(())
}

#[cfg(unix)]
#[test]
fn discover_symlink_counts_as_path_not_asset() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    std::fs::write(dir.path().join("AGENTS.md"), "x")?;
    std::os::unix::fs::symlink("AGENTS.md", dir.path().join("CLAUDE.md"))?;
    let found = discover(dir.path(), &Config::default())?;
    let rels: Vec<&str> = found.assets.iter().map(|a| a.rel_path.as_str()).collect();
    assert_eq!(rels, ["AGENTS.md"]);
    assert!(found.all_paths.contains("CLAUDE.md"));
    Ok(())
}
