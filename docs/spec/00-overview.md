# 00 — Overview

## Vision

Pulse (`pulse`) is an open-source, deterministic static analyzer and Quality Gate for **AI assets**: the instructions, agents, prompts, skills, tool definitions, MCP server configs and hooks that steer coding agents. Think "SonarQube for AI assets": one fast binary that runs in a pre-commit hook, an editor or CI, and answers three questions:

1. **Is it safe?** Hidden Unicode, prompt injection, leaked credentials, over-permissioned MCP servers and hooks, supply-chain risks.
2. **Is it lean?** Token budgets, always-on context load, duplication, bloat ("context debt").
3. **Is it clear and valid?** Contradictions, ambiguity, broken references, invalid frontmatter or config schemas.

Package managers (e.g. Microsoft `apm`) resolve and install AI assets. Pulse judges their quality.

## Asset families in scope

| Family | Examples |
|---|---|
| Instructions / rules | `CLAUDE.md`, `AGENTS.md`, `GEMINI.md`, `.github/copilot-instructions.md`, `.cursorrules`, `.cursor/rules/*.mdc`, `.claude/rules/*.md`, `.github/instructions/*.instructions.md`, `.windsurfrules`, `.clinerules`, `.kiro/steering/*.md` |
| Agents | `.claude/agents/*.md`, `.github/agents/*.agent.md`, `.github/chatmodes/*.chatmode.md` |
| Prompts / commands | `.claude/commands/*.md`, `.github/prompts/*.prompt.md`, `*.prompty` |
| Skills | `**/SKILL.md` (+ files it references) |
| Tools | tool-definition JSON (Anthropic `input_schema` / OpenAI `parameters` shapes) |
| MCP servers | `.mcp.json`, `.vscode/mcp.json`, `.cursor/mcp.json`, `claude_desktop_config.json`, `.gemini/settings.json`, `.kiro/settings/mcp.json` |
| Hooks & permissions | `.claude/settings*.json` (hooks, permissions), `.cursor/hooks.json`, plugin `hooks/hooks.json` |

The full discovery table is in `02-assets.md`.

## Principles

1. **Deterministic and offline.** Same input, same output. No network, no LLM in the core. (A semantic `--deep` mode is future work and opt-in; see ADR-002.)
2. **Fast.** Fast enough to run on every keystroke and every commit (see Performance budget).
3. **Precision over recall.** A noisy linter gets switched off. Heuristic rules default to lower severities and carry false-positive guards.
4. **Actionable.** Every finding has a rule ID, a location (path:line:col), a message and a remediation.
5. **Secure by construction.** Pulse reads untrusted text: linear-time regex only, no code execution, bounded file sizes, and Critical security findings cannot be suppressed from inside the scanned asset (ADR-006).
6. **Extensible.** Rules are small units behind one trait, registered in a static table and documented by convention. Users can add declarative custom rules in `pulse.toml`.

## Performance budget

Measured on an Apple M-series laptop, release build, warm FS cache:

| Scenario | Budget |
|---|---|
| Process start → exit (`pulse --version`) | < 2 ms |
| Typical repo (≤ 200 assets, ≤ 2 MB of assets, ≤ 20k files walked) | < 20 ms wall |
| Large monorepo (1,000 assets, 200k files walked) | < 150 ms wall |
| Peak RSS (typical repo) | < 30 MB |
| Release binary size | < 8 MB |

Tasks P2.8 and P7.1 add benchmarks that enforce these numbers. On other hardware, `scripts/bench.sh` scales the time budgets by the machine's speed relative to the reference machine, measured with a fixed calibration workload (ADR-038). Startup is judged net of the OS cost of spawning a process (1 ms of the 2 ms on the reference machine).

## Non-goals (v1)

- Running or simulating models; measuring runtime behavior.
- Resolving or installing packages (that is `apm`/npm territory). Pulse reads what is on disk.
- Exact tokenizer parity. Pulse uses a calibrated heuristic (ADR-005).
- Scanning arbitrary source code for prompts embedded in string literals. Possible later; out of v1.

## Glossary

- **Asset**: one discovered file with an `AssetKind`.
- **Finding / issue**: one rule violation at a location.
- **Always-on context**: tokens loaded into every agent session (root instructions, skill and agent descriptions, MCP tool schemas).
- **Context debt**: tokens that could be removed without losing information (over-budget tokens, duplicates, filler), as a ratio of total tokens.
- **Quality Gate**: the set of conditions that decides PASSED or FAILED.
