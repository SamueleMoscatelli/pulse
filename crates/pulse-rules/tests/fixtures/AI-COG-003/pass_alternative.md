# Tooling

- Don't use npm; use pnpm instead.
- Never call `fetch` directly, prefer the `api` client.
- Avoid default exports. Use named exports.
- Do not hand-roll retries; rather, wrap calls in `withRetry`.
