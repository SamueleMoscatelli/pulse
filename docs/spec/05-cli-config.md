# 05 — CLI & configuration

## Commands

```
pulse [GLOBAL] <COMMAND>

GLOBAL: -C, --root <DIR>   scan root (default: cwd)
        -c, --config <FILE> config path (default: <root>/pulse.toml if present)
        --color <auto|always|never>   (also honors NO_COLOR)
        -q, --quiet         findings only, no summary
        -v, --verbose       show the remediation of every finding (terminal)
        --timings           print phase timings to stderr

pulse init [--force]                        write a commented default pulse.toml (refuse to overwrite without --force)
pulse scan [PATHS...] [OPTIONS]             analyze and report; exit 0 unless --fail-on-gate
    -f, --format <terminal|json|sarif|markdown|github>   (default terminal)
    -o, --output <FILE>                     write the report to a file instead of stdout
    --fail-on-gate                          exit 1 when the gate fails
    --min-severity <info|low|medium|high|critical>   hide findings below this level (display only; the gate still sees all)
    --rule <ID>...  / --skip-rule <ID>...   run only / skip rules (repeatable; prefix match: AI-SEC)
    --new-only                              only findings not in the baseline
    --changed-since <GIT-REF>               only assets changed vs ref: `git diff --name-only --relative <ref>...HEAD`
                                              plus untracked files (`git ls-files --others --exclude-standard`), run in the root
pulse ci [same options as scan]             = scan --fail-on-gate; format defaults to `github` when GITHUB_ACTIONS=true,
                                              else terminal with color off
pulse baseline [--output FILE]              write current fingerprints to .pulse/baseline.json
pulse rules [--category <CAT>] [--format terminal|json|markdown]   list rules (id, effective severity, enabled, name);
                                              CAT is a name or ID prefix (security|SEC, …); markdown is the README
                                              rule table (ID linked to docs/rules/<ID>.md, severity or `off`, name, summary)
pulse explain <RULE-ID>                     print the rule's docs (embedded docs/rules/<ID>.md; ID is case-insensitive,
                                              headings bold when colored); unknown ID → exit 2 with a suggestion
```

`PATHS` are relative to the scan root (absolute paths under it are accepted) and restrict analysis to those files/dirs, but project rules still see the full discovered asset set for context (e.g. reference checks).

Color: an explicit `--color always|never` wins over `NO_COLOR`; `auto` means stdout is a TTY and `NO_COLOR` is unset or empty (`pulse ci` treats `auto` as off). `--timings` and walk warnings go to stderr.

Exit codes: `0` OK / gate passed · `1` gate failed · `2` usage, config or I/O error.

## `pulse.toml` (default file written by `pulse init`)

```toml
# Pulse configuration — https://github.com/SamueleMoscatelli/pulse
# Every key is optional; these are the defaults.

[scan]
include = []                 # extra globs to treat as assets, e.g. ["prompts/**/*.md"]
exclude = []                 # globs to skip, e.g. ["examples/**"]
max_file_bytes = 1048576
follow_symlinks = false

# Declare additional asset kinds for custom paths
# [[scan.custom]]
# glob = "prompts/**/*.txt"
# kind = "prompt"            # instructions | scoped_instructions | agent | prompt | skill | tools | mcp_config | settings | hooks
# load = "on_demand"         # always | conditional | on_demand

[context]
window = 200000              # model context window used for context_load
calibration = 1.0            # multiply token estimates (calibrate against your tokenizer)

[budgets]                    # tokens unless noted
instructions = 3000
scoped_instructions = 1500
agent = 2500
prompt = 2000
skill = 5000
tools = 3000
custom = 3000
always_on = 8000
description = 150
mcp_servers = 8              # count
tools_count = 40             # count

[gate]
fail_on_severity = "high"
max_context_debt = 0.10
max_always_on_tokens = 0
new_code_only = false
# min_ratings = { security = "A", reliability = "B" }

[rules]
# Per-rule toggles and severity overrides:
# "AI-COG-003" = "off"
# "AI-COG-004" = "medium"
#
# Per-rule options:
# [rules.AI-SEC-010]
# allowed_domains = ["github.com", "docs.rs"]
# [rules.AI-COG-001]
# exclusive = [["bun", "deno"]]
# [rules.AI-SEC-002]
# allow = ["ignore previous instructions"]   # phrases to accept (e.g. in security docs)

# Suppress findings by path (config suppressions work for every rule, including critical security rules)
# [[ignore]]
# path = "docs/attacks/**"
# rules = ["AI-SEC-002", "AI-SEC-011"]
# reason = "Documented attack samples"

# Custom declarative rules
# [[custom_rules]]
# id = "X-ORG-001"
# name = "No internal hostnames"
# severity = "high"
# kinds = ["instructions", "skill"]
# pattern = '\binternal\.corp\.example\b'
# message = "Internal hostname leaked into AI context"
# remediation = "Use the public alias"
# One matcher per rule: `pattern` (regex on source), `phrases` (list, matched on
# normalized text at word boundaries) or `max_tokens` (fires above this many tokens).
# Severity overrides and "off" in [rules] apply to custom rule IDs too.
```

Validation: unknown top-level keys and unknown rule IDs are errors (exit 2) with a "did you mean" suggestion. Custom rule IDs must match `^X-[A-Z0-9]+-\d{3}$`. Unknown rule IDs are checked by the engine against the registry; `pulse-core` checks `max_context_debt` in [0, 1], budgets, `context.window` and `context.calibration` > 0. Optional keys: `scan.custom.load` (omitted = the kind's default), `ignore.rules` and `custom_rules.kinds` (omitted = all), `ignore.reason`, `custom_rules.remediation`. Each custom rule needs exactly one of `pattern`, `phrases` (non-empty) or `max_tokens` (> 0); IDs must be unique; `[rules.X-…]` tables accept only `severity`/`enabled`. An invalid regex is an engine config error (exit 2) naming the rule ID. `gate.min_ratings` keys: `security`, `reliability`, `clarity`, `efficiency`.

## Inline suppressions (Markdown-based assets)

```markdown
<!-- pulse-ignore AI-COG-004 -- reason -->          suppresses on the next non-blank line
Some text <!-- pulse-ignore AI-EFF-007 -->           trailing form: suppresses on the same line
<!-- pulse-ignore-file AI-EFF-001 -- reason -->     whole file; must appear before the first heading
```

- Several IDs can be comma-separated. `all` is not accepted (be explicit).
- Rules with `inline_suppressible = false` (AI-SEC-001/002/003/008/012) ignore inline suppressions and emit an Info note `suppression ignored: <ID> can only be suppressed in pulse.toml` (ADR-006).
- JSON assets have no inline suppressions; use `[[ignore]]`.
- An unused suppression comment produces an Info finding (`AI-STR-002`, message "unused suppression"): `unused suppression: <ID>`, only for rules that ran. A comment listing no IDs, `all`, an unknown non-`X-` ID, or a `pulse-ignore-file` after the first heading produces an Info `invalid suppression: …` and suppresses nothing (ADR-016).
- A next-line comment skips blank lines and lines holding only another suppression comment (so comments stack). A suppression matches findings that *start* on its target line. Comments inside code blocks are not directives. IDs may be separated by commas or spaces.
- Order: inline suppressions, then `[[ignore]]` (which can also drop the AI-STR-002 notes), then dedup. `[[ignore]]` globs match the scan-root-relative path (`*` does not cross `/`), and unknown IDs in `ignore.rules` are config errors.
