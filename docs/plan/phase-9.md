# Phase 9 — Kiro support (v1.2.0)

Goal: Pulse understands Kiro the way it understands Copilot and Claude Code: steering files with
their four inclusion modes, `#[[file:…]]` references, skills, MCP config, hooks (both formats),
custom agents (JSON and Markdown), and powers distributed as libraries. Measured on real repos
before anything is released.

Kiro docs (checked 2026-10-31; cite the exact pages in the ADRs): steering
<https://kiro.dev/docs/steering/>, skills <https://kiro.dev/docs/skills/>, MCP
<https://kiro.dev/docs/mcp/configuration/>, hooks <https://kiro.dev/docs/hooks/> and
<https://kiro.dev/docs/hooks/types/>, custom agents
<https://kiro.dev/docs/custom-agents/configuration-reference/>, powers
<https://kiro.dev/docs/powers/create/>. Real repos use formats the docs no longer describe (legacy
`*.kiro.hook` files, Markdown agents with frontmatter, `POWER.md`): support what is in use, not only
what is documented.

Corpus, cloned next to this repo:
- `affaan-m/ECC` (`../ECC`, MIT) pinned at `b38be6a` (2026-10-29): a live `.kiro/` with 22 steering
  files (always/fileMatch/manual), 43 skills, 33 JSON + 33 Markdown agents, 14 `*.kiro.hook` hooks,
  `settings/mcp.json.example`.
- `mehtadman87/kiro-config` (`../kiro-config`, MIT) pinned at `89f77b9` (2026-04-03): a power
  (`POWER.md`) with 31 steering files, 24 `*.kiro.hook` hooks and specs at the root, not under
  `.kiro/`.

Copy snippets into fixtures (keep them minimal, MIT); never make tests depend on the clones. Don't
copy from `kirodotdev-labs/awesome-kiro` (CC BY-NC). Every card reports its before/after delta from
`scripts/compare.sh` on both repos in the PR description.

## P9.1 — Kiro corpus baseline
- **Size** S · **Depends** P8.8
- **Read** `docs/eval/awesome-copilot.md` (layout to follow), `scripts/compare.sh --help`
- **Do** Clone both corpus repos at the pinned commits. Record the v1.1.0 baseline in `docs/eval/kiro.md` (reproduce steps, totals, per-rule counts per repo, assets found by kind), in the same layout as `docs/eval/awesome-copilot.md`. List what v1.1.0 misses per Kiro file type (expected: every `.kiro/` steering, hook, agent and MCP file; skills are found through `**/SKILL.md` but attributed to the wrong assistants), so later cards can show their delta against it.
- **Done when** the baseline file is committed; `scripts/compare.sh --new "$(command -v pulse)"` on both repos reports zero deltas.

## P9.2 — Kiro as an assistant: discovery and load modes
- **Size** M · **Depends** P9.1
- **Read** `scripts/spec.sh "Discovery table"`, `scripts/spec.sh "Metrics"`, ADR-042, ADR-045, ADR-048
- **Do** Add `Assistant::Kiro` (id `kiro`, name `Kiro`; it fills the 8-bit `AssistantSet`, keep the compile-time assert). Load table (ADR-042, amend it in a new ADR-049 with the docs URLs):
  - `.kiro/steering/**/*.md` → ScopedInstructions, loaded by Kiro. Load mode from the frontmatter `inclusion`: `always` or absent → Always; `fileMatch` → Conditional; `manual` → OnDemand; `auto` → OnDemand, with `description` always loaded, like a skill description.
  - `.kiro/skills/**/SKILL.md` → loaded by Kiro only. `AGENTS.md` (root and nested) → add Kiro to its readers.
  - `--assistant kiro` works through the existing selection.
  - AI-STR-008 near-misses: misspelled `.kiro/` folders (`.kiro/steerings/`, `.kiro/skill/`, `.kiro/hook/`, `.kiro/agent/`), with the folder to move to.
  - README "supported assistants" text and `pulse init` comments mention Kiro.
- **Not now** frontmatter validation and `#[[file:]]` references (P9.3); hooks, MCP, agents (P9.4–P9.6).
- **Done when** check is green; fixtures cover each inclusion mode's load mode, a steering file with no frontmatter, a Kiro skill, nested `AGENTS.md` for Kiro, each near-miss; the CONTEXT LOAD block shows a Kiro row on ECC with only its `always` steering, AGENTS.md and skill descriptions; `compare.sh` delta in the PR.

## P9.3 — Steering frontmatter and file references
- **Size** M · **Depends** P9.2
- **Read** `scripts/spec.sh "AI-STR-001"`, `scripts/spec.sh "AI-STR-003"`, `scripts/spec.sh "AI-EFF-002"`, `scripts/spec.sh "AI-COG-006"`, the `add-rule` skill
- **Do**
  - AI-STR-001 schema "Kiro steering": known keys `inclusion`, `fileMatchPattern`, `name`, `description`; `inclusion` must be one of the four modes; `fileMatch` needs `fileMatchPattern` (string or list of strings); `auto` needs `name` and `description`. The frontmatter must start on the first line (the docs require it); a later `---` block is reported.
  - References: `#[[file:<path>]]`, `#[[file:<path>:<line>]]`, `#[[file:<path>:<start>-<end>]]`, `#[[folder:<path>]]`. Paths resolve from the workspace root for `.kiro/steering/` files and from the containing folder for `AGENTS.md`. AI-STR-003 reports missing targets (and line ranges past the end of the file). The import graph counts a referenced file toward the always-on tokens of an `always` steering file, as `@` imports do for AI-EFF-002 (whole file; a line range counts only those lines).
  - AI-COG-006 runs on `auto` steering descriptions (they route like skill descriptions).
- **Done when** check is green; fixtures for each schema case, each reference form (present, missing, bad range, folder), and an `auto` description that routes weakly; `compare.sh` delta in the PR.

## P9.4 — Kiro MCP config
- **Size** S · **Depends** P9.2
- **Read** `scripts/spec.sh "Per-kind parsed views"`, `scripts/spec.sh "AI-SEC-004"`, `scripts/spec.sh "AI-SEC-007"`, `scripts/spec.sh "AI-EFF-005"`
- **Do** `.kiro/settings/mcp.json` → McpConfig, loaded by Kiro (`loaded_by` row, so `--assistant` works). The MCP view reads Kiro's per-server keys: `disabled`, `autoApprove`, `disabledTools`, and remote `url`/`headers`/`oauth`. A `disabled: true` server is not counted by AI-EFF-005, but security rules still run on it (its secrets are still committed). `autoApprove` containing `"*"` (or every tool of the server) is over-permissive: report it with AI-SEC-007, amending the rule's spec to cover MCP auto-approval. `mcp.json.example` and similar are not discovered (as now).
- **Done when** check is green; fixtures for a Kiro MCP file, a disabled server with a secret (reported) and with sprawl (not counted), wildcard and explicit `autoApprove`; `compare.sh` delta in the PR.

## P9.5 — Kiro hooks, legacy and current
- **Size** M · **Depends** P9.2
- **Read** `scripts/spec.sh "AI-STR-006"`, `scripts/spec.sh "AI-SEC-008"`, `scripts/spec.sh "AI-SEC-009"`, ADR-046 (how the Copilot hook dialect was added)
- **Do** Discover `.kiro/hooks/*.kiro.hook` and `.kiro/hooks/*.json` as Hooks, loaded by Kiro. Two dialects in the settings parser:
  - legacy IDE file (`*.kiro.hook`, the format in use in the corpus): `name`, `version`, `enabled`, `description`, `when { type, patterns }`, `then { type: askAgent | runCommand, prompt | command, timeout }`;
  - current file (`version: "v1"`, `hooks: [ { name, trigger, matcher, action { type: command | agent, command | prompt }, timeout, enabled, confirm } ]`), triggers from the hook-types page (`PromptSubmit`, `AgentStop`/`Stop`, `SessionStart`, `SessionEnd`, `AgentSpawn`, `PreToolUse`, `PostToolUse`, `FileCreate`, `FileSave`/`PostFileSave`, `FileDelete`, `PreTaskExecution`, `PostTaskExecution`, `Manual`; take the exact spellings from the docs and cite them in the ADR).
  - Commands feed AI-SEC-008/009 like other hooks; AI-STR-006 reports unknown trigger or `when.type`/`then.type` values, a command action without `command`, an agent action without `prompt`, and a `matcher` that is not a valid regex. A disabled hook is still checked, but its findings say it is disabled.
  - Library: `*.kiro.hook` anywhere outside `.kiro/hooks/` is a library copy (OnDemand, ADR-041 semantics).
- **Not now** prose rules (injection, exfiltration) on agent-action prompts inside JSON: backlog.
- **Done when** check is green; fixtures for each dialect (valid, unknown trigger, missing command/prompt, bad regex, remote code in a command, disabled), a library `*.kiro.hook`; ECC's 14 and kiro-config's 24 hook files are assets; `compare.sh` delta in the PR.

## P9.6 — Kiro custom agents, JSON and Markdown
- **Size** L · **Depends** P9.4, P9.5
- **Read** `scripts/spec.sh "AI-SEC-007"`, `scripts/spec.sh "AI-STR-005"`, `scripts/spec.sh "AI-STR-001"`, `scripts/spec.sh "Per-kind parsed views"`
- **Do** Discover `.kiro/agents/*.json` and `.kiro/agents/*.md` as Agent, loaded by Kiro (OnDemand: the docs don't say descriptions are always in context; record that in the ADR).
  - JSON view: `name`, `description`, `prompt` (inline, or `file://` relative to the agent file), `tools`, `allowedTools`, `toolsSettings`, `permissions.rules`, `resources` (`file://`, `skill://`), `hooks` (inline, both forms), `mcpServers`, `includeMcpJson`, `model`.
  - AI-SEC-007: unrestricted shell (`allowedTools` with `shell`/`execute_bash`/`*`, `tools: ["*"]` with no `permissions`, or a `permissions` rule allowing every shell command); write access to `~/.kiro` called out per the docs' warning.
  - AI-STR-005: unknown tool names, from Kiro's built-in tool list and category tags (`read`, `write`, `shell`, `web`, `subagent`, `@builtin`, `@server`, `@server/tool`); cite the list's URL.
  - AI-STR-001: Markdown agent frontmatter schema (keys from the docs plus those in use in the corpus: `name`, `description`, `tools`, `allowedTools`, `model`, …); JSON agents with legacy `allowedTools` and no `permissions` get a finding that CLI V3 skips the profile until it is upgraded (`/upgrade-agent`).
  - AI-STR-003: missing `file://` targets in `prompt` and `resources`. AI-COG-006 on agent descriptions. Inline `hooks` and `mcpServers` go through the P9.4/P9.5 checks.
  - AI-EFF-001 budgets count the JSON agent's prompt text.
- **Not now** prose rules on inline JSON `prompt` strings (their spans live inside JSON): backlog.
- **Done when** check is green; fixtures for JSON and Markdown agents (valid, unrestricted shell, unknown tool, legacy `allowedTools`, missing `file://`, weak description); ECC's 66 agent files are assets; `compare.sh` delta in the PR.

## P9.7 — Kiro powers as libraries
- **Size** M · **Depends** P9.3, P9.5
- **Read** ADR-041, `scripts/spec.sh "Discovery table"`, `scripts/spec.sh "pulse.toml"`
- **Do** A folder holding `POWER.md` (legacy) or `plugin.json` with `keywords` (current Agent Plugins format) is a power root. Its `steering/*.md`, `hooks/*.kiro.hook` and `hooks/*.json`, `mcp.json` and `skills/**/SKILL.md` are Kiro library assets (OnDemand, every rule runs, ADR-041 semantics), so a power repo like kiro-config gets the same checks as a live `.kiro/`. `POWER.md` itself: an Instructions-like library asset, so text rules run on it. Without a power root, a `steering/` folder at the repo root is not an asset (no false discovery in unrelated repos).
- **Done when** check is green; fixtures for a `POWER.md` power and a `plugin.json` power (steering, hooks, mcp, skills), and a bare `steering/` folder that stays undiscovered; kiro-config's steering and hooks are assets; always-on tokens unchanged; `compare.sh` delta in the PR.

## P9.8 — Evaluation report and release
- **Size** S · **Depends** P9.6, P9.7
- **Read** `docs/eval/kiro.md`, `docs/plan/post-release.md` ("Every release from now on")
- **Do** Run `scripts/compare.sh` (v1.1.0 vs `main`) on both Kiro repos, `../awesome-copilot`, this repo and `tests/fixtures/repos/*`. Write the result to `docs/eval/kiro.md` under "v1.2.0 candidate": deltas per rule, a verdict on 20 random new Kiro findings, and any regressions on the non-Kiro repos (fix small ones here, put the rest in the backlog). Add the Kiro section to the CHANGELOG draft. Then stop and hand over to the user, who installs the build and tests it. Only after the user approves: the `1.2.0-rc.1` release PR, then 1.2.0, per the release routine. Ask before pushing any tag, and before moving `v1`.
- **Done when** the report is committed; the user has approved the comparison; the rc PR is open (or the card stops at the report, with what the user asked to change added as cards).
