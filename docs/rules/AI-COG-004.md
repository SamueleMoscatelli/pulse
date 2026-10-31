# AI-COG-004 — Emphasis inflation

**Severity:** Low · **Category:** Cognitive · **Applies to:** Markdown assets · **Inline-suppressible:** yes

## Why it matters
Caps, bold and "!!" tell the model which rules matter most. When most rules are shouted, none stands out, and the rules that really are critical lose their weight.

## What triggers it
- A directive is emphasized when it contains `IMPORTANT`, `CRITICAL`, `MUST` or `NEVER` in caps, two or more `!`, is entirely bold, or has an ALL-CAPS word of 4+ letters that is not a known acronym (`JSON`, `HTTP`, `YAML`, `README`, `TODO`, …). Words in code spans don't count. Bold used as structure is not emphasis: table cells, bold labels ending in `:` (`**Output (…):**`) and ALL-CAPS words in a `**Term**:` lead-in (`**WARNING**: …`; `IMPORTANT`, `CRITICAL`, `MUST` and `NEVER` still count).
- Reported once per file, at the first emphasized directive, when more than 25% of its directives are emphasized and there are at least 4 of them. The message lists the first emphasized lines: "Emphasis on 4 of 6 directives (lines 3, 4, 5, …); emphasis works only when rare (max 25%)".

## Noncompliant
```markdown
- IMPORTANT: run `pnpm test` before every commit.
- You MUST use pnpm.
- **Never push directly to main.**
- Always add a changelog entry!!
- Keep functions short.
```

## Compliant
```markdown
- NEVER push directly to main.
- Run `pnpm test` before every commit.
- Use pnpm.
- Add a changelog entry.
- Keep functions short.
```

## Options
None.

## Suppressing
`<!-- pulse-ignore AI-COG-004 -- reason -->` (Markdown) or a `[[ignore]]` entry in `pulse.toml`.
