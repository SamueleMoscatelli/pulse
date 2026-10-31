# Pulse

[![CI](https://github.com/SamueleMoscatelli/pulse/actions/workflows/ci.yml/badge.svg)](https://github.com/SamueleMoscatelli/pulse/actions/workflows/ci.yml)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

**A static analyzer and Quality Gate for AI assets**: the instructions, agents, prompts,
skills, tool definitions, MCP server configs and hooks that steer coding agents. Think
"SonarQube for AI assets": one deterministic, offline binary that scans a repository in
milliseconds and runs the same way in a pre-commit hook, an editor or CI.

Pulse answers three questions:

1. **Is it safe?** Hidden Unicode, prompt injection, leaked credentials, over-permissioned
   MCP servers and hooks, supply-chain risks.
2. **Is it lean?** Token budgets, always-on context load, duplication, bloat ("context debt").
3. **Is it clear and valid?** Contradictions, vague directives, broken references, invalid
   frontmatter or config schemas.

It understands Claude Code (`CLAUDE.md`, agents, commands, rules, skills, settings),
GitHub Copilot (`copilot-instructions.md`, `*.instructions.md`, `*.prompt.md`, agents, chat
modes), Kiro (`.kiro/steering/` with its inclusion modes, `.kiro/skills/`), `AGENTS.md`,
`GEMINI.md`, Cursor, Windsurf and Cline rules, hook configs, MCP configs
(`.mcp.json`, `.vscode/mcp.json`, `claude_desktop_config.json`, …) and tool definitions;
other paths can be declared in `pulse.toml`.

## Stability

Pulse follows [semantic versioning](https://semver.org/). Within a major version, these do not
change incompatibly:

- commands, flags and exit codes
- `pulse.toml` keys and inline suppression syntax
- rule IDs (a removed rule's ID is never reused)
- the JSON report schema (`pulse/report@1`) and the SARIF output

Minor releases can add rules and improve detections, so an upgrade may report new findings: pin
a version in CI (`version: 1.1.0` in the Action, `rev:` in pre-commit) and upgrade on purpose.
The terminal output is for people and may change in any release.

## Quickstart

```sh
pulse scan                 # analyze the current repository
pulse scan -v              # same, with the fix for every finding (default: medium and higher)
pulse init                 # write a commented pulse.toml with every default
pulse ci                   # scan + enforce the Quality Gate (exit 1 when it fails)
pulse explain AI-SEC-002   # what a rule checks and how to fix it
pulse rules                # every rule with its effective severity
```

Reports: `--format terminal|json|sarif|markdown|github`. Accept today's findings with
`pulse baseline`, then gate only on new ones with `pulse ci --new-only`; `--changed-since <ref>`
limits the scan to changed files, and `--assistant copilot` (or `claude-code`, `cursor`, `kiro`,
…) to the files one assistant loads. Exit codes: `0` passed, `1` gate failed, `2` usage, config
or I/O error.

### Sample output

```text
$ pulse scan
 Pulse 1.2.0-rc.1 · 33 assets · 204 tokens · 10 ms

 QUALITY GATE   FAILED
   ✖ severity ≥ high        1 issue    (allowed 0)
   ✔ context debt           0%         (max 10%)

 CONTEXT LOAD   Always-on context: the tokens each assistant reads on every request
                                          tokens  of 200k window  of 8k budget
   Copilot         █░░░░░░░░░░░░░░░░░░░       54              0%          0.7%
   Claude Code     █░░░░░░░░░░░░░░░░░░░       48              0%          0.6%
   Cursor          █░░░░░░░░░░░░░░░░░░░       35              0%          0.4%
   Windsurf        █░░░░░░░░░░░░░░░░░░░       35              0%          0.4%
   Cline           █░░░░░░░░░░░░░░░░░░░       32              0%          0.4%
   Gemini CLI      █░░░░░░░░░░░░░░░░░░░       32              0%          0.4%
   Codex           █░░░░░░░░░░░░░░░░░░░       32              0%          0.4%
   Kiro            █░░░░░░░░░░░░░░░░░░░       32              0%          0.4%
   Always loaded   agent.tools.json 15 · tools/fetch.json 14 · 3 agent descriptions 7 · CLAUDE.md 6 · … (Copilot)
   On demand       90 tokens in skills, agents, prompts and scoped rules, loaded only when used
   Budget          8k always-on tokens per assistant (`budgets.always_on` in pulse.toml). Bars show the share of the 200k context window; they turn yellow above 75% of the budget and red above it.

   Security      A   0 vulnerabilities
   Reliability   D   1 bug
   Clarity       C   4 smells
   Efficiency    A   context debt 0% · share of tokens that is avoidable waste (duplicates, filler, over-budget excess)

   CRITICAL 0   HIGH 1   MEDIUM 4   LOW 0   INFO 0

 .claude/agents/planner.md
   3:1     MEDIUM    AI-COG-006  Weak routing description "Plans work.": 2 words (< 20), no trigger cue
                     ↳ Say what it covers; add "Use when …" with the requests, keywords and file types that should trigger it. Template: "<What it does>. Use when <requests, keywords, file types>. [Not for <what a sibling handles>.]"

 tools/fetch.json
   1:1     HIGH      AI-STR-002  Invalid shape: `tools` must be an array, found object
                     ↳ Fix the syntax or shape so the agent (and Pulse) can read the whole file.
```

### Configuration and suppressions

Every `pulse.toml` key is optional: rule toggles and severity overrides (`"AI-COG-003" = "off"`),
token budgets, gate thresholds, extra asset globs, path ignores (`[[ignore]]`) and declarative
custom rules (`[[custom_rules]]`). Run `pulse init` to see them all. In Markdown assets a finding
can be suppressed inline, with a reason:

```markdown
<!-- pulse-ignore AI-COG-004 -- the emphasis is intentional here -->
```

## Metrics

**Context load** is what the assistant receives on every request, before your prompt: root
instructions (`CLAUDE.md`, `AGENTS.md`, `.github/copilot-instructions.md`, …), rules applied to
every file, tool definitions, and the name and description of every skill and agent (their
bodies load only when used). It matters because it is paid on every turn: it costs money, adds
latency, leaves less of the window for the task itself, and dilutes the instructions that count
among the ones that don't. Each assistant loads its own files, so Pulse adds them up per
assistant (one line each when they differ) and judges the largest; the bar is colored against
`budgets.always_on` (8k tokens by default), which AI-EFF-002 enforces. "On demand" is the
rest: skill and agent bodies, prompts and scoped rules, loaded only when a request needs them.

**Context debt** is the share of tokens that is avoidable waste: the token waste of open
efficiency findings (duplicated content, filler phrasing, the excess over a budget) divided by
the total tokens of all scanned assets. Overlapping waste counts once.

**Ratings** go from A (best) to E. Security, Reliability (structure) and Clarity (cognitive)
take the worst open severity in their category (none or info A, low B, medium C, high D,
critical E); Efficiency follows context debt (≤ 5% A, ≤ 10% B, ≤ 20% C, ≤ 50% D, else E).

**The Quality Gate** passes when every condition holds; by default no open finding of severity
high or above, and context debt at most 10%. `[gate]` in `pulse.toml` can add minimum ratings
and a cap on always-on tokens. `pulse ci` exits with 1 when the gate fails.

## Install

Release binaries for macOS (arm64, x64), Linux (x64, arm64; static musl) and Windows (x64)
are attached to each [GitHub release](https://github.com/SamueleMoscatelli/pulse/releases).
From source: `cargo install --locked --git https://github.com/SamueleMoscatelli/pulse pulse-cli`.

```sh
# macOS / Linux
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/SamueleMoscatelli/pulse/releases/latest/download/pulse-cli-installer.sh | sh
# Windows (PowerShell)
powershell -ExecutionPolicy Bypass -c "irm https://github.com/SamueleMoscatelli/pulse/releases/latest/download/pulse-cli-installer.ps1 | iex"
```

Each release also carries a Homebrew formula (`pulse.rb`) for the
`SamueleMoscatelli/homebrew-tap` tap.

## GitHub Action

```yaml
permissions:
  contents: read
  security-events: write   # only with `sarif: true`
steps:
  - uses: actions/checkout@v7
  - uses: SamueleMoscatelli/pulse@v1  # the action; follows every 1.x release
    with:
      version: latest        # the binary; pin one, e.g. 1.1.0, for stable CI
      args: --new-only       # extra `pulse ci` flags
      sarif: true            # also upload a SARIF report to code scanning
```

The action downloads the release binary for the runner, verifies its checksum and runs
`pulse ci` (findings as annotations, the report in the job summary, failure when the gate fails).

## pre-commit

```yaml
repos:
  - repo: https://github.com/SamueleMoscatelli/pulse
    rev: v1.1.0
    hooks:
      - id: pulse
```

The hook runs `pulse ci` on the whole repository and needs `pulse` on `PATH`.

## Rules

Each rule has a page under [`docs/rules/`](docs/rules/) (also printed by `pulse explain <ID>`).
This table is generated by `pulse rules --format markdown` and kept in sync by a test.

<!-- rules:start -->
| Rule | Severity | Name | Summary |
|---|---|---|---|
| [AI-COG-001](docs/rules/AI-COG-001.md) | medium | Contradictory directives | Two directives in the file contradict each other. |
| [AI-COG-002](docs/rules/AI-COG-002.md) | low | Vague directive | A directive uses a vague term with nothing concrete to pin it down. |
| [AI-COG-003](docs/rules/AI-COG-003.md) | info | Negative-only constraint | A vague prohibition says what not to do without saying what to do instead. |
| [AI-COG-004](docs/rules/AI-COG-004.md) | low | Emphasis inflation | Too many directives are emphasized, so emphasis no longer stands out. |
| [AI-COG-005](docs/rules/AI-COG-005.md) | medium | Cross-file contradiction | Two instruction files that load together contradict each other. |
| [AI-COG-006](docs/rules/AI-COG-006.md) | medium | Weak routing description | A skill, agent or auto steering description is too short, lacks a trigger cue, or is in first person. |
| [AI-COG-007](docs/rules/AI-COG-007.md) | low | Unstructured long instructions | Long instructions have too few headings, or a paragraph is a wall of text. |
| [AI-EFF-001](docs/rules/AI-EFF-001.md) | high | Asset over token budget | Asset is larger than the token budget for its kind. |
| [AI-EFF-002](docs/rules/AI-EFF-002.md) | high | Always-on context over budget | The context loaded into every session is larger than budgets.always_on. |
| [AI-EFF-003](docs/rules/AI-EFF-003.md) | medium | Duplicated content | The same paragraph or line appears more than once in the context. |
| [AI-EFF-004](docs/rules/AI-EFF-004.md) | medium | Inlined bulk content | A large code block, table or data dump is inlined in a loaded asset. |
| [AI-EFF-005](docs/rules/AI-EFF-005.md) | medium | MCP server/tool sprawl | Too many MCP servers or tools are enabled, or a server is defined twice. |
| [AI-EFF-006](docs/rules/AI-EFF-006.md) | medium | Oversized skill or agent | A skill body is too long, or a skill/agent description exceeds its budget. |
| [AI-EFF-007](docs/rules/AI-EFF-007.md) | low | Filler phrasing | Filler phrases spend tokens without adding instructions. |
| [AI-SEC-001](docs/rules/AI-SEC-001.md) | critical | Invisible or bidirectional Unicode | File contains invisible, bidi-control or tag characters that hide text from reviewers. |
| [AI-SEC-002](docs/rules/AI-SEC-002.md) | critical | Prompt-injection phrase | File contains a phrase typical of prompt-injection attacks. |
| [AI-SEC-003](docs/rules/AI-SEC-003.md) | critical | Hardcoded secret | File contains what looks like a real API key, token, password or private key. |
| [AI-SEC-004](docs/rules/AI-SEC-004.md) | high | MCP credential not externalized | An MCP server credential (env, header or CLI flag) is written as a literal value. |
| [AI-SEC-005](docs/rules/AI-SEC-005.md) | high | Unpinned MCP server package | MCP server package is not pinned to an exact version. |
| [AI-SEC-006](docs/rules/AI-SEC-006.md) | high | Insecure MCP transport | A remote MCP server is reached over plain http://. |
| [AI-SEC-007](docs/rules/AI-SEC-007.md) | high | Over-permissive agent permissions | Agent permissions allow any shell command, any URL, or skip approval prompts. |
| [AI-SEC-008](docs/rules/AI-SEC-008.md) | critical | Remote code execution in hook | A hook or shell snippet downloads code and runs it (curl \| bash). |
| [AI-SEC-009](docs/rules/AI-SEC-009.md) | high | Unsafe interpolation in hook command | A hook command expands tool input unquoted or executes it. |
| [AI-SEC-010](docs/rules/AI-SEC-010.md) | high | Data-exfiltration pattern | Content tells the agent to send secrets or context to an outside URL. |
| [AI-SEC-011](docs/rules/AI-SEC-011.md) | high | Hidden instructions | Content the model reads but a reviewer may not see (comments, hidden HTML, base64). |
| [AI-SEC-012](docs/rules/AI-SEC-012.md) | critical | Tool description poisoning | A tool or skill description carries instructions aimed at the model, not the user. |
| [AI-SEC-013](docs/rules/AI-SEC-013.md) | medium | Sensitive path reference | Asset references a credential store or private file (SSH keys, .env, cloud credentials). |
| [AI-STR-001](docs/rules/AI-STR-001.md) | high | Invalid or missing frontmatter | Frontmatter is unterminated, lacks a required key, or has an unknown key. |
| [AI-STR-002](docs/rules/AI-STR-002.md) | high | Invalid file content | File is not valid UTF-8, JSON, YAML frontmatter, or has the wrong shape for its kind. |
| [AI-STR-003](docs/rules/AI-STR-003.md) | medium | Broken reference or import | An @import, link, skill path or Kiro #[[file:…]] points at a file that does not exist. |
| [AI-STR-004](docs/rules/AI-STR-004.md) | medium | Name / description constraint violated | Skill name or description breaks the platform's format or length limits. |
| [AI-STR-005](docs/rules/AI-STR-005.md) | medium | Unknown tool in allow-list | An allow-list names a tool that does not exist, so the grant silently does nothing. |
| [AI-STR-006](docs/rules/AI-STR-006.md) | high | Invalid hook definition | A hook has an unknown event or type, no command, a broken matcher or script path. |
| [AI-STR-007](docs/rules/AI-STR-007.md) | high | Duplicate asset name | Two assets of one kind and platform share an invocation name; one shadows the other. |
| [AI-STR-008](docs/rules/AI-STR-008.md) | medium | Asset not loaded from its location | An asset sits where no assistant loads it, so it silently does nothing. |
<!-- rules:end -->

## Release

Releases are built by [`dist`](https://opensource.axo.dev/cargo-dist/) (`dist-workspace.toml`,
`.github/workflows/release.yml`): pushing a `vX.Y.Z` tag builds every target and publishes a
GitHub release; pull requests run `dist plan` only. After changing the dist config, run
`dist generate` and commit the regenerated workflow.

To release: bump the workspace version and `CHANGELOG.md` in a PR, merge it, then tag `main`.
A `-rc.N` suffix publishes a prerelease. After a stable release, move the major tag that
GitHub Action users reference:

```sh
git tag -a vX.Y.Z -m "Pulse X.Y.Z" && git push origin vX.Y.Z
git tag -f -a v1 -m "Pulse v1" 'vX.Y.Z^{}' && git push -f origin v1
```

## Build

```sh
cargo build --release          # binary at target/release/pulse
cargo run -q -p pulse-cli -- --help
scripts/check.sh               # fmt + clippy + tests
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). False positives are bugs: please
[report them](https://github.com/SamueleMoscatelli/pulse/issues/new/choose) with the snippet
that triggered the rule. Security issues: see [SECURITY.md](SECURITY.md).

## License

Licensed under the [Apache License, Version 2.0](LICENSE).
