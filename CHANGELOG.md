# Changelog

All notable changes to Pulse are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and Pulse follows
[semantic versioning](https://semver.org/).

## 1.1.0-rc.1 - 2026-10-31

Release candidate for 1.1.0: fewer false positives, clearer advice and full coverage of
library repos, measured on `github/awesome-copilot` (`docs/eval/awesome-copilot.md`).

### Added

- Library repos: `*.agent.md`, `*.chatmode.md`, `*.prompt.md` and `*.instructions.md` are
  found at any path, and `hooks.json` / `mcp.json` anywhere by content; `scan.library` marks
  distribution folders.
- AI-STR-008: assets in a place no assistant loads (misspelled folders and names, library
  copies), with the location to move them to.
- `--assistant <NAME>` scans only the files the selected assistants load and reports what it
  left out and why (`-v` lists them; JSON `scope`).
- CONTEXT LOAD block: always-on tokens per assistant, one bar each with tokens, share of the
  window and share of the `budgets.always_on` budget, plus what always loads and what loads on
  demand. JSON adds `metrics.context_load_by_assistant`.
- Fixes (`↳`) shown by default for medium and higher findings; `-v` shows them for all.
- Terminal output wraps long lines at the window width, indented under their own column.
- `scripts/compare.sh`: before/after comparison of two builds on real repos.

### Changed

- AI-COG-003 reports only weak prohibitions instead of every negative-only constraint.
- Always-on metrics, the `max_always_on_tokens` gate and AI-EFF-002 use the assistant that
  loads the most instead of a sum over every assistant's files.

### Fixed

- False positives found on the corpus: AI-STR-001 (current Copilot agent keys such as
  `user-invocable` and `agents`), AI-STR-003 (paths shown only as code), AI-STR-005/006
  (Copilot agents and hooks read as Claude ones), AI-SEC-003 (URLs and word paths),
  AI-COG-002/004 (list lead-ins, bold labels and table cells), AI-EFF-003 (duplicate lines
  now Low).

## 1.0.0 - 2026-10-25

The first stable release.

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

## 1.0.0-rc.1 - 2026-10-25

Release candidate for 1.0.0, with the same features.
