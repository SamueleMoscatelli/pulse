# Pulse

Static analyzer and Quality Gate for AI assets: instructions, agents, prompts,
skills, tools, MCP configs and hooks. Deterministic, offline and fast.

## Status

Early development (Phase 0: foundations). Commands are stubs and do not analyze anything yet.

## Build

```sh
cargo build --release          # binary at target/release/pulse
cargo run -q -p pulse-cli -- --help
scripts/check.sh               # fmt + clippy + tests
```

Licensed under Apache-2.0.
