# Release checklist

Tag the release from `main`.

- Never push to `main` directly.
- Do not edit `Cargo.lock` by hand.
- Don't commit `.env` files.
- Never print secrets from `config.toml`.
- Do not skip the `cargo test` step.
- Avoid `unsafe` blocks.
- You must not bump the version past `1.x`.
- Never delete tags older than 30 days.
- Do not use `--force` on shared branches.
