# Phase 8 — Real-world accuracy (v1.1.0)

Goal: fewer false positives, clearer advice, and full coverage of library repos, measured against
v1.0.0 on a real corpus before anything is released.

Corpus: `github/awesome-copilot`, cloned next to this repo (`../awesome-copilot`), pinned at commit
`0fe6545b` (2026-10-28). Copy snippets into fixtures (MIT-licensed, keep them minimal); never
make tests depend on the clone. Every card reports its before/after delta from
`scripts/compare.sh` in the PR description.

## P8.1 — Before/after comparison harness
- **Size** S · **Depends** P7.3
- **Read** `scripts/spec.sh "JSON ("`
- **Do** `scripts/compare.sh [--old BIN] [--new BIN] <repo>...`: OLD defaults to `pulse` on `PATH` (v1.0.0 from the release installer), NEW to `target/release/pulse` (built first with `cargo build --release -p pulse-cli`). It runs `scan -f json` with both on each repo and prints Markdown:
  - totals (assets, findings by severity, gate result, context load) old → new
  - per-rule finding counts old → new → delta, sorted by |delta|
  - added and removed findings matched by `fingerprint`, grouped by rule, at most 10 per rule (path:line, message), with the full lists written to `target/compare/<repo-name>.md`
  Bash plus `python3` (stdlib only) for the JSON work, like the other scripts' style. Record the v1.0.0 baseline for the corpus in `docs/eval/awesome-copilot.md` (commit, per-rule counts, totals) so later cards and the user can compare against it.
- **Done when** check is green; `scripts/compare.sh --new "$(command -v pulse)" ../awesome-copilot` reports zero deltas (same binary on both sides); the baseline file is committed.

## P8.2 — Recognize library assets outside activation folders
- **Size** M · **Depends** P8.1
- **Read** `scripts/spec.sh "Discovery table"`, `scripts/spec.sh "Content sniffing"`, `scripts/spec.sh "Per-kind parsed views"`, `scripts/spec.sh "pulse.toml"`
- **Do** Discover `*.agent.md`, `*.chatmode.md` (Agent), `*.prompt.md` (Prompt) and `*.instructions.md` (ScopedInstructions) at any path, after the existing platform-specific rows. Discover `hooks.json` anywhere when its top level has a `hooks` object, and `mcp.json` anywhere when it has `mcpServers` or `servers` (content-sniffed like Tools). Assets found only by these new rows are not loaded from where they are: never `Always` for context-load and AI-EFF-002 totals (OnDemand), whatever their frontmatter says; every rule still runs on them. Add `[scan] library = []` (globs of folders that deliberately store assets for distribution, e.g. `["agents/**", "instructions/**", "skills/**"]`); assets under them are treated the same way, including `SKILL.md` files. `pulse init` documents the key. Spec: amend the discovery table and the `pulse.toml` section; ADR-041.
  - Corpus check: `agents/` (222), `instructions/` (193), `hooks/*/hooks.json` (8) and the root `mcp.json` become assets; always-on tokens must not grow.
- **Done when** check is green; fixtures cover each new row, a sniffed-out `hooks.json`/`mcp.json` that is not one, the load mode of a library `*.instructions.md` with `applyTo: '**'`, and `scan.library`; `compare.sh` delta in the PR.

## P8.3 — AI-STR-008: asset in a place no assistant loads
- **Size** M · **Depends** P8.2
- **Read** `scripts/spec.sh "Discovery table"`, `scripts/spec.sh "Structure (STR)"`, the `add-rule` skill
- **Do** New rule AI-STR-008 "Asset not loaded from its location" (Medium, Structure, so it counts as a bug: the asset silently does nothing). First, build the table of load locations for each supported assistant (Copilot in VS Code and the coding agent, Claude Code, Cursor, Windsurf, Cline, Gemini CLI, Codex) from their current docs, and cite the URLs in ADR-042; note which locations are configurable (VS Code `chat.*Locations`). Report:
  - assets found only by the P8.2 suffix rows, outside `scan.library` and outside plugin folders (an ancestor with `plugin.json` or `.claude-plugin/plugin.json`): one finding per folder, not per file ("222 agents in `agents/` are not loaded by any assistant: move them to `.github/agents/`, or declare the folder in `[scan] library` if it is a library");
  - `SKILL.md` not directly inside `<skills-dir>/<name>/` (e.g. `.claude/skills/SKILL.md`, `.github/skills/a/b/SKILL.md`);
  - near-miss names and folders: `.github/agent/`, `.github/instruction/`, `.github/prompt/`, `.claude/agent/`, `.claude/command/`, `.claude/skill/`, `skill.md`/`Skill.md`, `copilot-instruction.md`, `copilot-instructions.md` outside `.github/`, `claude.md`/`Claude.md` (works on macOS and Windows but not on Linux CI), `*.mdc` outside `.cursor/rules/`, `*.agent.md` directly in `.github/` instead of `.github/agents/`. The message names the expected location.
  Discovering near-miss files is a cheap name check during the walk (no content reads for files that do not match).
- **Done when** check is green; a fail fixture per case above and pass fixtures for plugin folders, `scan.library` and correctly placed assets; on the corpus, with `[scan] library = ["agents/**", "instructions/**", "skills/**", "hooks/**"]`, AI-STR-008 reports nothing unexpected (list the hits in the PR); `compare.sh` delta in the PR.

## P8.4 — AI-COG-003 redesign: real prohibitions only
- **Size** M · **Depends** P8.3
- **Read** `scripts/spec.sh "AI-COG-003"`, `scripts/spec.sh "AI-COG-002"`, the `add-rule` skill
- **Do** Prohibitions are specific guardrails worth keeping; report only the weak cases (user feedback, ADR-043):
  - A prohibition is an imperative negation opening a clause: `Do not`, `Don't`, `Never`, `Avoid`, `You must not`/`should not`. Not prohibitions: statements of fact ("They do not have a `plugin.json`"), purpose clauses ("To avoid …"), `no X` noun phrases and labels ("**No Authentication**", "no build step"), quoted or code-span text, headings.
  - Concrete prohibitions are fine: the object contains a code span, path, command, quoted literal or number ("Do not add `x-awesome…`", "Never commit `.azure/`").
  - Report (1) a vague prohibition (negation + `lexicon::VAGUE` term or a short list: unnecessary, too much, excessive, verbose, bad, wrong) with no positive alternative nearby, at Info, aggregated as today; (2) once per asset, at Low, when prohibitions are ≥ 60% of its directives and ≥ 8: "mostly prohibitions: lead with what good output looks like, then the limits".
  - Update the rule doc, spec section and fixtures from the corpus samples (the sampled false positives become pass fixtures).
- **Done when** check is green; every listed non-prohibition has a pass fixture; both triggers have fail fixtures; `compare.sh` shows AI-COG-003 well below 379 on the corpus, and the PR lists 15 random remaining hits with a verdict.

## P8.5 — Visible advice: fixes by default, `-v` for all
- **Size** M · **Depends** P8.4
- **Read** `scripts/spec.sh "Terminal (default)"`, `scripts/spec.sh "AI-COG-006"`, `scripts/spec.sh "AI-EFF-003"`, `scripts/spec.sh "AI-COG-004"`
- **Do**
  - Terminal: show the `↳` fix under every Medium, High and Critical finding by default; `-v` shows it for all. When fixes are hidden (not `-v`, not `-q`), say so in two places: in the summary block, right under the severity counts (`Fixes shown for medium and higher · run pulse scan -v to see all`), and at the end of the report with: `↳ Fixes shown for medium and higher findings. Run with -v to see fixes for all <N> findings, or \`pulse explain <RULE-ID>\` for a rule's full guidance.` Update `-v`'s `--help` ("Show the fix for every finding (by default only for medium and higher)"), the README Quickstart (`pulse scan -v`) and the spec's terminal sample.
  - AI-COG-006: per-finding fix naming what is missing (too short → say what it covers; no trigger → add "Use when …" with the requests, keywords and file types that should trigger it; first person → third person), plus the template `<What it does>. Use when <requests, keywords, file types>. <Not for <what a sibling handles>>.` Add a "Writing a description that routes well" section to `docs/rules/AI-COG-006.md` with the corpus before/after (`agentic-workflows`).
  - AI-EFF-003 duplicate lines: full path of the other file and why it matters ("Line also in `.github/copilot-instructions.md:89`; both load on every request, so it is paid twice (~N tokens)"); one finding per file pair listing its lines.
  - AI-COG-004: "Emphasis on 4 of 8 directives (lines 325, 326, …); emphasis works only when rare (max 25%): keep it for the rules that must never be missed".
- **Done when** check is green; terminal snapshots cover the summary hint and the footer (shown, hidden with `-v`, hidden with `-q`, absent when nothing is hidden); message snapshots for the three rules; `compare.sh` delta in the PR (counts unchanged except by design).

## P8.6 — Context load front and center
- **Size** M · **Depends** P8.5
- **Read** `scripts/spec.sh "Metrics"`, `scripts/spec.sh "Terminal (default)"`, `scripts/spec.sh "Markdown ("`, `scripts/spec.sh "AI-EFF-002"`, ADR-042
- **Do**
  - Terminal: move context load out of the ratings list into its own block right under the gate, e.g.
    ```
     CONTEXT LOAD   42.7k tokens on every request · 21.3% of a 200k window
       ██████░░░░░░░░░░░░░░░░░░░░░░░░ 21%   (budget 8k)
       Always loaded   AGENTS.md 12.1k · .github/copilot-instructions.md 8.4k · 465 skill descriptions 18.0k · …
       On demand       1.0M tokens in skills, agents and prompts, loaded only when used
    ```
    with the bar colored green/yellow/red against `budgets.always_on`. Same block in the Markdown and GitHub summary.
  - Per assistant: using ADR-042's load table, compute always-on tokens for each assistant whose assets are present (Copilot, Claude Code, Cursor, …) and show one line each when there is more than one; today's single total adds up files no single assistant loads together. The gate, AI-EFF-002 and `context_load` use the largest one. JSON: `metrics.context_load_by_assistant`. ADR-044.
  - Context debt: label it everywhere as what it is: "context debt 21.9% · share of tokens that is avoidable waste (duplicates, filler, over-budget excess)".
  - README: a "Metrics" section explaining context load (tokens the assistant receives on every request before your prompt, why it matters: cost, latency, less room for the task, diluted instructions), context debt (Σ token waste of efficiency findings ÷ total tokens), the ratings and the gate.
- **Done when** check is green; terminal and Markdown snapshots show the block (one assistant, several assistants, nothing always-on); JSON schema test covers the new field; README section present; `compare.sh` delta in the PR.

## P8.7 — False-positive guards from the corpus
- **Size** M · **Depends** P8.6
- **Read** `scripts/spec.sh "AI-COG-002"`, `scripts/spec.sh "AI-COG-004"`, `scripts/spec.sh "AI-STR-003"`, `scripts/spec.sh "AI-EFF-003"`, the `add-rule` skill
- **Do** One pass fixture per corpus case:
  - AI-COG-002: a sentence ending in `:` directly followed by a list or table is anchored (`.github/skills/code-review/SKILL.md` "The appropriate evidence depends on the resource:").
  - AI-COG-004: bold used as structure is not emphasis: table cells, a bold label ending in `:` (`**Output (…):**`), a `**Term**: …` lead-in (`skills/agent-supply-chain/SKILL.md`).
  - AI-STR-003 skill paths: a path mentioned only in a code span or code block is reported only if its first directory exists in the skill or root (a likely typo) or it is run by a command (`python`, `python3`, `node`, `bash`, `sh`, `pwsh`, `uv run`, …); Markdown links and `@` imports are checked as today (`.github/skills/create-canvas-extension/SKILL.md`, `assets/preview.png` at lines 23 and 64). Amend ADR-027 with ADR-045.
  - AI-EFF-003: duplicate *lines* (not paragraphs) are Low; duplicate paragraphs stay Medium.
  - AI-STR-005: know Copilot tool names in `*.agent.md`/`*.prompt.md` `tools:` (`codebase`, `search`, `fetch`, `runCommands`, `problems`, toolsets, `<server>/*` MCP refs, …); 1598 corpus hits after P8.2 (`agents/*.agent.md`).
  - AI-STR-006: accept the Copilot hooks format (`version: 1`, camelCase events such as `sessionStart`/`preToolUse`, `bash`/`powershell` commands, `timeoutSec`); 24 corpus hits after P8.2 (`hooks/*/hooks.json`).
- **Done when** check is green; each case has a pass fixture and a near-miss fail fixture; `compare.sh` delta in the PR.

## P8.8 — Evaluation report and release candidate
- **Size** S · **Depends** P8.7
- **Read** `docs/eval/awesome-copilot.md`, `docs/plan/post-release.md` ("Every release from now on")
- **Do** Run `scripts/compare.sh` (v1.0.0 vs `main`) on the corpus, this repo and `tests/fixtures/repos/*`. Write the result to `docs/eval/awesome-copilot.md` under "v1.1.0 candidate": deltas per rule, a verdict on 20 random new findings and 20 random removed ones, and any regressions (fix small ones here, put the rest in the backlog). Then stop and hand over to the user, who installs the build (`cargo install --path crates/pulse-cli --locked`) and tests it. Only after the user approves: the `1.1.0-rc.1` release PR per the release routine. Ask before pushing any tag.
- **Done when** the report is committed; the user has approved the comparison; the rc PR is open (or the card stops at the report, with what the user asked to change added as cards).
