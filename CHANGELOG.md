# Changelog

All notable changes to Pulse are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and Pulse follows
[semantic versioning](https://semver.org/).

## 1.0.0-rc.1 - 2026-10-25

The first public release candidate.

### Added

- `pulse scan`, `pulse ci` (Quality Gate with exit codes), `pulse init`, `pulse baseline`,
  `pulse rules` and `pulse explain`.
- Discovery of Claude Code, GitHub Copilot, `AGENTS.md`/`GEMINI.md`, Cursor, Windsurf and Cline
  assets, MCP configs, hook configs and tool definitions, plus custom globs in `pulse.toml`.
- 34 rules in four categories: security (AI-SEC), structure (AI-STR), efficiency (AI-EFF) and
  cognitive clarity (AI-COG), each documented under `docs/rules/`.
- Declarative custom rules (`[[custom_rules]]`), per-rule severity overrides and options, path
  ignores and inline suppressions.
- Ratings (A–E), context-load and context-debt metrics, and a configurable Quality Gate.
- Baselines (`--new-only`) and changed-file scans (`--changed-since`).
- Terminal, JSON, SARIF 2.1.0, Markdown and GitHub (annotations + job summary) reports.
- Release binaries for macOS, Linux and Windows, shell and PowerShell installers, a Homebrew
  formula, a GitHub Action and a pre-commit hook.
