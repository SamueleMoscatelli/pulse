# AI-EFF-007 — Filler phrasing

**Severity:** Low · **Category:** Efficiency · **Applies to:** Markdown assets · **Inline-suppressible:** yes

## Why it matters
Phrases like "please note that" or "in order to" add tokens without adding instructions. Instruction files are loaded on every request, so that waste is paid on every request, and padding makes the real rules harder to spot.

## What triggers it
- Filler phrases, matched case-insensitively and across Markdown emphasis: "please note that", "it is important to note", "make sure to always" (→ "always"), "you are a helpful assistant", "as an AI", "in order to" (→ "to"), "feel free to", "basically", "very very" (→ "very"), "due to the fact that" (→ "because"), and similar.
- With 3 or more hits in a file, one Low finding at the first hit lists the most frequent phrases. With fewer, each hit is reported at Info.
- Text in code blocks is not checked.
- The wasted tokens are the phrase's tokens minus the replacement's tokens, summed over all hits.

## Noncompliant
```markdown
Please note that the project uses pnpm. In order to run the tests, use `pnpm test`.
Feel free to ask if something is basically unclear.
```

## Compliant
```markdown
The project uses pnpm. To run the tests, use `pnpm test`.
Ask if something is unclear.
```

## Options
None.

## Suppressing
`<!-- pulse-ignore AI-EFF-007 -- reason -->` (Markdown) or a `[[ignore]]` entry in `pulse.toml`.
