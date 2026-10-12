We are building **Pulse** (`pulse`), an open-source, high-performance static analysis engine and Quality Gate for AI primitives (prompts, system instructions like `CLAUDE.md` and `.cursorrules`, and `mcp.json` tool definitions). Think of Pulse as the **"SonarQube for AI Assets"**.

### Project Vision & Objective
Package managers like Microsoft's `apm` handle asset resolution, but developers and enterprises lack a static analysis tool to scan AI context for:
1. **Security & Vulnerabilities** (Indirect prompt injections, zero-width unicode Trojans, credential leakage, over-permissioned MCP tools).
2. **Context Efficiency & Bloat** (Token usage budgeting, redundant instructions, AST/comment bloat, context window ratios).
3. **Cognitive & Quality Rules** (Rule contradictions, instruction ambiguity, negative constraint adherence).

### Core Goals for This Initialization Phase
1. **Language & Architecture:** Go (single binary compilation, high speed, <30ms execution).
2. **CLI Experience:** Commands should include `pulse init`, `pulse scan` (terminal analysis output), and `pulse ci` (Quality Gate with exit codes).
3. **Rule Architecture:** A modular, extensible scanner pipeline (`pkg/analyzer`) where rules are structured like SonarQube rules (ID, Category, Severity, Line, Message, Remediation).

---

### Key Requirements to Implement

Please create the initial repository structure and core Go codebase containing:

1. **Project Directory Layout:**
   - `cmd/pulse/main.go` — CLI interface (use Cobra or a lightweight CLI framework).
   - `pkg/analyzer/` — Core analysis engine and rule definitions (`security.go`, `efficiency.go`, `cognitive.go`).
   - `pkg/parser/` — Markdown/JSON AST and token estimation utilities.
   - `pkg/report/` — Sonar-style terminal report formatter and Quality Gate logic.
   - `pkg/config/` — Config parser for `pulse.toml`.

2. **Core Scan Rules for MVP:**
   - **`AI-SEC-001` (Critical):** Scan for invisible Unicode/Bidi Trojan overrides (`U+200B`, `U+200C`, `U+200D`, `U+202E`).
   - **`AI-SEC-002` (Critical):** Scan for indirect prompt injection keywords (`"ignore previous instructions"`, `"system override"`).
   - **`AI-EFF-001` (High):** Calculate token budget consumption (heuristic estimation ~4 chars/token) and flag assets exceeding a threshold (e.g., >3,000 tokens).
   - **`AI-COG-001` (Medium):** Detect direct instruction contradictions (e.g., "always use TypeScript" vs "use plain JavaScript").

3. **Sonar-Style Terminal Output:**
   - Print a clear matrix showing Quality Gate Status (`PASSED` / `FAILED`), Ratings (A-E), Issue breakdown by severity, and Context Debt ratio.
   - Support a `--fail-on-gate` flag that returns exit code `1` on Critical/High findings.

4. **Configuration File (`pulse.toml`):**
   - Provide a default generator via `pulse init` with token thresholds and rule toggles.

Please write clean, well-tested Go code, set up `go.mod` (module name: `github.com/your-org/pulse`), and include unit tests for the security and efficiency parsers. Let's start by scaffolding the project!