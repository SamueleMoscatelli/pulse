# AI-COG-001 — Contradictory directives

**Severity:** Medium · **Category:** Cognitive · **Applies to:** Markdown assets · **Inline-suppressible:** yes

## Why it matters
When one file tells the model both to do something and not to do it, or to use two tools that exclude each other, the model has to pick one, and different runs pick differently. You get inconsistent output and spend tokens on instructions that cancel out.

## What triggers it
- Two directives with opposite polarity about the same thing: "Always use semicolons" and "Never use semicolons". A directive is a sentence or list item with a marker such as `always`, `must`, `should`, `use`, `prefer`, `do` or `never`, `must not`, `don't`, `avoid`, `no`.
- Two positive directives that pick different members of a mutually exclusive set: TypeScript vs JavaScript, tabs vs spaces, npm/yarn/pnpm/bun, single vs double quotes, jest/vitest/mocha, snake_case vs camelCase, and more.
- Not reported:
  - text in code blocks or HTML comments;
  - directives scoped to different contexts ("Use npm for scripts" / "Use pnpm for the workspace");
  - directives in sections whose headings name different scopes ("Python" / "Makefile");
  - a negative directive that only narrows a positive one ("Always use TypeScript" / "Never use TypeScript enums").

The finding is placed on the second directive and quotes the first one with its line number.

## Noncompliant
```markdown
## Conventions
Always use TypeScript.

## Style
Use plain JavaScript.
```

## Compliant
```markdown
## Frontend
Always use TypeScript.

## Build scripts
Use plain JavaScript.
```

## Options
`[rules.AI-COG-001]`
- `exclusive`: extra sets of mutually exclusive choices, matched like the built-in ones (case-insensitive, plural `s` ignored), e.g. `exclusive = [["bun", "deno"], ["react", "vue"]]`.

## Suppressing
`<!-- pulse-ignore AI-COG-001 -- reason -->` (Markdown) or a `[[ignore]]` entry in `pulse.toml`.
