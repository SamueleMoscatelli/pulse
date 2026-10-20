# AI-COG-003 — Negative-only constraint

**Severity:** Info · **Category:** Cognitive · **Applies to:** Markdown assets · **Inline-suppressible:** yes

## Why it matters
"Don't mock the database" tells the model what to avoid but not what to do, so it has to guess the alternative. Models follow positive instructions more reliably, and naming the alternative removes the guess.

## What triggers it
- A negative directive (`never`, `don't`, `do not`, `must not`, `avoid`, `No …`) with an object, when neither that sentence nor the next one offers an alternative: `instead`, `rather`, `prefer`, or `use`/`using` not directly negated (`never use` doesn't count).
- `;` splits sentences, so "Don't use npm; use pnpm" passes.
- Code blocks, HTML comments and headings are not checked.
- With 3 or more hits in a file, one Info finding at the first hit lists them. With fewer, each hit is reported.

## Noncompliant
```markdown
- Don't mock the database in integration tests.
```

## Compliant
```markdown
- Don't mock the database in integration tests; use the `testdb` fixture instead.
```

## Options
None.

## Suppressing
`<!-- pulse-ignore AI-COG-003 -- reason -->` (Markdown) or a `[[ignore]]` entry in `pulse.toml`.
