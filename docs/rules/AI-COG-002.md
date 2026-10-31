# AI-COG-002 — Vague directive

**Severity:** Low · **Category:** Cognitive · **Applies to:** Markdown assets · **Inline-suppressible:** yes

## Why it matters
"Handle errors appropriately" tells the model nothing it would not do anyway: it fills in "appropriately" with its own defaults, which may not be yours. A concrete value, command, path or example turns a wish into a rule the model can follow and you can check.

## What triggers it
- A directive (a sentence with `always`, `must`, `should`, `never`, `avoid`, … or opening with a verb such as "Write", "Use", "Handle") that contains a vague term: "appropriate(ly)", "as needed", "properly", "best practices", "clean code", "etc.", "and so on", "some", "reasonable", "if necessary", "good", "nice".
- Not reported when the same sentence has a concrete anchor: a number, a code span, a path or file name, or an example ("e.g.", "for example", "such as"), or a list or table right after it when it ends in `:` ("Use the appropriate evidence for each resource:" followed by the list).
- Descriptive sentences ("Some modules predate this guide") are not directives and are not checked.
- With 3 or more hits in a file, one Low finding at the first hit lists the most frequent terms. With fewer, each hit is reported at Info.

## Noncompliant
```markdown
- Handle errors appropriately.
- Use reasonable timeouts.
```

## Compliant
```markdown
- Wrap errors in `AppError` and log them with the request ID.
- Use a 5-second timeout for HTTP calls.
```

## Options
None.

## Suppressing
`<!-- pulse-ignore AI-COG-002 -- reason -->` (Markdown) or a `[[ignore]]` entry in `pulse.toml`.
