# Pulse

Static analyzer and Quality Gate for AI assets: instructions, agents, prompts,
skills, tools, MCP configs and hooks. Deterministic, offline and fast.

## Status

Pre-1.0: the rule set and report formats may still change between releases.

## Install

Release binaries for macOS (arm64, x64), Linux (x64, arm64; static musl) and Windows (x64)
are attached to each [GitHub release](https://github.com/SamueleMoscatelli/pulse/releases).

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
  - uses: actions/checkout@v4
  - uses: SamueleMoscatelli/pulse@v0.1.0
    with:
      version: latest        # or a release, e.g. 0.1.0
      args: --new-only       # extra `pulse ci` flags
      sarif: true            # also upload a SARIF report to code scanning
```

The action downloads the release binary for the runner, verifies its checksum and runs
`pulse ci` (findings as annotations, the report in the job summary, failure when the gate fails).

## pre-commit

```yaml
repos:
  - repo: https://github.com/SamueleMoscatelli/pulse
    rev: v0.1.0
    hooks:
      - id: pulse
```

The hook runs `pulse ci` on the whole repository and needs `pulse` on `PATH`.

## Release

Releases are built by [`dist`](https://opensource.axo.dev/cargo-dist/) (`dist-workspace.toml`,
`.github/workflows/release.yml`): pushing a `vX.Y.Z` tag builds every target and publishes a
GitHub release; pull requests run `dist plan` only. After changing the dist config, run
`dist generate` and commit the regenerated workflow.

## Build

```sh
cargo build --release          # binary at target/release/pulse
cargo run -q -p pulse-cli -- --help
scripts/check.sh               # fmt + clippy + tests
```

Licensed under Apache-2.0.
