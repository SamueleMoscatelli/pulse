//! Synthesizes the `large` fixture repo (spec 07-testing → Performance checks).
//!
//! Shared by `benches/scan.rs` and `examples/gen_repo.rs` via `#[path]`; deterministic.

use std::fs;
use std::io;
use std::path::Path;

/// Assets per package: CLAUDE.md, AGENTS.md, an agent, a command and a skill.
const PER_PACKAGE: usize = 5;

/// Writes `assets` AI assets and enough non-asset files to reach `files` in total.
pub fn generate(root: &Path, assets: usize, files: usize) -> io::Result<()> {
    let packages = assets.div_ceil(PER_PACKAGE).max(1);
    let fillers = files.saturating_sub(assets);
    let mut written = 0;
    for p in 0..packages {
        let pkg = root.join(format!("packages/p{p:04}"));
        let docs: [(&str, &str); PER_PACKAGE] = [
            ("CLAUDE.md", "instructions"),
            ("AGENTS.md", "agent instructions"),
            (".claude/agents/reviewer.md", "agent"),
            (".claude/commands/fix.md", "command"),
            ("skills/release/SKILL.md", "skill"),
        ];
        for (i, (rel, what)) in docs.iter().enumerate() {
            if written == assets {
                break;
            }
            let path = pkg.join(rel);
            if let Some(dir) = path.parent() {
                fs::create_dir_all(dir)?;
            }
            fs::write(&path, asset_text(p, i, what))?;
            written += 1;
        }
        // Spread the fillers evenly over packages, 100 per directory.
        let share = fillers / packages + usize::from(p < fillers % packages);
        for f in 0..share {
            let dir = pkg.join(format!("src/m{:03}", f / 100));
            if f.is_multiple_of(100) {
                fs::create_dir_all(&dir)?;
            }
            fs::write(dir.join(format!("f{f:05}.rs")), FILLER)?;
        }
    }
    Ok(())
}

const FILLER: &str = "pub fn answer() -> u32 {\n    42\n}\n";

/// About 1.5 KB of markdown; every 10th asset also carries an always/never contradiction.
fn asset_text(package: usize, index: usize, what: &str) -> String {
    let frontmatter = match index {
        2 => "---\nname: reviewer\ndescription: Reviews diffs for correctness.\n---\n",
        3 => "---\ndescription: Fix the failing test.\n---\n",
        4 => "---\nname: release\ndescription: Cut a release of the package.\n---\n",
        _ => "",
    };
    let conflict = if (package * PER_PACKAGE + index).is_multiple_of(10) {
        "\n## Style\n\n- Always use tabs for indentation.\n- Never use tabs for indentation.\n"
    } else {
        ""
    };
    format!(
        "{frontmatter}# Package p{package:04} {what}\n\n\
         This package implements part of the billing pipeline. Read the module docs\n\
         before changing public APIs, and keep changes small and reviewable.\n\n\
         ## Build\n\n\
         ```sh\ncargo build -p p{package:04}\ncargo test -p p{package:04}\n```\n\n\
         ## Conventions\n\n\
         - Prefer explicit error types over strings.\n\
         - Keep functions under fifty lines where practical.\n\
         - Document every public item with a one-line summary.\n\
         - Run the formatter before committing.\n\n\
         ## Testing\n\n\
         Unit tests live next to the code. Integration tests live under `tests/`.\n\
         When a test is flaky, file an issue instead of retrying it in CI.\n\
         Snapshot tests use insta; review every pending snapshot before accepting it.\n\n\
         ## Architecture\n\n\
         The crate exposes a small facade over the ledger, the invoicing engine and\n\
         the notification queue. Each component owns its storage and talks to the\n\
         others through typed messages. Avoid shared mutable state across components.\n\
         {conflict}"
    )
}
