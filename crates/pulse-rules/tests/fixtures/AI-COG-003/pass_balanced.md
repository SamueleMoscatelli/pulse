# Conventions

- Write tests for every public function.
- Keep functions under 40 lines.
- Name branches `task/<id>-<slug>`.
- Run `scripts/check.sh` before committing.
- Use `thiserror` in libraries.
- Prefer `memchr` for byte search.
- Document every rule in `docs/rules/`.
- Never push to `main` directly.
- Do not edit `Cargo.lock` by hand.
- Don't commit `.env` files.
- Never print secrets from `config.toml`.
- Do not skip the `cargo test` step.
- Avoid `unsafe` blocks.
- You must not bump the version past `1.x`.
- Never delete tags older than 30 days.
