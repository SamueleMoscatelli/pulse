# AI-COG-005 — Cross-file contradiction

**Severity:** Medium · **Category:** Cognitive · **Applies to:** instruction files (project scope) · **Inline-suppressible:** yes

## Why it matters
Instruction files that load into the same session add up to one prompt. If the root `CLAUDE.md` says "use pnpm" and `packages/x/CLAUDE.md` says "use npm", the model gets both while it works in `packages/x`, and different runs pick differently. Each file looks fine when you read it alone, so nobody spots the conflict in review.

## What triggers it
The AI-COG-001 contradiction checks (same directives, exclusive sets and false-positive guards), applied across files that load together:
- **Always-on set**: root instructions that load in every session (`CLAUDE.md`, `AGENTS.md`, `.github/copilot-instructions.md`, …), rules promoted to always-on (`alwaysApply: true`, `.claude/rules` without `paths`), and every file they `@import`.
- **Directory chains**: each nested `CLAUDE.md` / `AGENTS.md` with the always-on set and the nested instruction files in its ancestor directories, plus their imports.

Not reported:
- contradictions inside a single file (AI-COG-001 reports those);
- nested files in unrelated directories (`packages/a` vs `packages/b`);
- glob-scoped rules (`globs`, `paths`, `applyTo`), which load only for matching files;
- the guarded cases from AI-COG-001: different scoping qualifiers, sections whose headings name different scopes, a negative directive that only narrows a positive one. A file's title (its first heading, at level 1) does not count as a section scope here.

The finding goes on the directive in the file that loads later (the nested one, or the later path in the always-on set), and quotes the earlier one with its path and line.

## Noncompliant
```markdown
<!-- CLAUDE.md -->
Use pnpm.

<!-- packages/x/CLAUDE.md -->
Use npm.
```

## Compliant
```markdown
<!-- CLAUDE.md -->
Use pnpm for the workspace.

<!-- packages/x/CLAUDE.md -->
Use npm for the release scripts.
```

## Options
None of its own. The extra exclusive sets in `[rules.AI-COG-001] exclusive` apply here too.

## Suppressing
`<!-- pulse-ignore AI-COG-005 -- reason -->` on the reported directive, or a `[[ignore]]` entry in `pulse.toml`.
