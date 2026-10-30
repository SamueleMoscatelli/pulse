# AI-COG-003 — Negative-only constraint

**Severity:** Info · **Category:** Cognitive · **Applies to:** Markdown assets · **Inline-suppressible:** yes

## Why it matters
Specific prohibitions are useful guardrails: "Never commit `.env`" leaves nothing to guess. A vague one ("Don't add unnecessary comments") tells the model what to avoid but not where the line is, so it has to guess the alternative. And an asset that is mostly prohibitions never shows what good output looks like; models follow positive instructions more reliably.

## What triggers it
- A prohibition is an imperative negation opening a clause (the sentence start or after a `Label:`): `Do not`, `Don't`, `Never`, `Avoid`, `You must not`, `You should never`. Statements of fact ("They do not have a manifest", "You do not need to restart"), purpose clauses ("To avoid …"), `No X` labels ("**No Authentication**", "no build step"), quoted or code-span text, code blocks, HTML comments and headings are not prohibitions.
- **Vague prohibition** (Info): the object has a vague term (`unnecessary`, `too much`, `excessive`, `verbose`, `bad`, `wrong`, `appropriate`, `properly`, `best practices`, …) before any `when`/`unless`/`because` clause, nothing concrete (a code span, quoted literal, path, number or file name), and neither that sentence nor the next one offers an alternative (`instead`, `rather`, `prefer`, `focus`, `use` not directly negated, or a later `: do X` / `and do X` clause). With 3 or more hits in a file, one finding at the first hit lists them.
- **Mostly prohibitions** (Low, once per file): at least 8 prohibitions, making up 60% or more of the file's directives.

## Noncompliant
```markdown
- Don't add unnecessary comments.
```

## Compliant
```markdown
- Don't add comments that restate the code; comment only on why a choice was made.
- Never commit `.env` files.
```

## Options
None.

## Suppressing
`<!-- pulse-ignore AI-COG-003 -- reason -->` (Markdown) or a `[[ignore]]` entry in `pulse.toml`.
