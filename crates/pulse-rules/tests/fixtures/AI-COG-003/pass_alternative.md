# Tooling

- Don't write verbose logs; log one line per request instead.
- Never leave bad names, prefer the domain term.
- Avoid unnecessary allocations. Use `memchr` for byte search.
- Do not add excessive retries; rather, fail fast and report.
- Never put wrong data in messages: redact them.
- Avoid unnecessary complexity and focus on the essential elements.
- Do not require executable tests when a manual evaluation is more appropriate.
