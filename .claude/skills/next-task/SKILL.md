---
name: next-task
description: Implement the next open roadmap task of Pulse end to end (read card, implement, verify, tick, commit). Pass a task ID (e.g. P2.3) to run a specific one.
argument-hint: "[task-id]"
disable-model-invocation: true
---

# /next-task

Implement exactly **one** roadmap task, then stop. Be economical with context: read only what the card points to.

## 1. Load the task
- Sync first: `git switch main && git pull --ff-only`. If a previous task's PR is still open (its task isn't ticked on `main`), stop and tell the user to merge it first.
- If `$ARGUMENTS` is a task ID, run `scripts/task.sh show $ARGUMENTS`; otherwise run `scripts/task.sh next`.
- If the card's **Depends** tasks are not ticked in `docs/plan/ROADMAP.md`, stop and tell the user.
- If `git log -1` fails (no commits yet), first commit the existing workspace: `git add -A && git commit -m "chore: workspace setup"`.
- If `git status --porcelain` shows uncommitted changes that are not yours, stop and ask the user how to proceed.
- Create the task branch: `git switch -c task/<ID>-<short-slug>` (e.g. `task/P0.2-core-model`). Never commit task work to `main`.

## 2. Gather only the needed context
- Run each `scripts/spec.sh "<query>"` listed under **Read**. Load the skills it names (e.g. `add-rule`).
- Read existing code only where you will integrate: start from the crate's `lib.rs`/`mod.rs` and the types you'll touch. Use `rg` to locate symbols instead of opening whole directories.
- Don't read other phase files, the full spec or `target/`.

## 3. Implement
- Follow **Do**. Stay inside the card's scope; **Not now** items are off-limits.
- If the spec is ambiguous, pick the simplest interpretation that satisfies **Done when**, and record it (step 5).
- If the spec is wrong or impossible (e.g. a crate API differs), adapt minimally, then amend the spec section and add an ADR.
- Write tests along with the code. Use `scripts/check.sh -p <crate>` for the inner loop.

## 4. Verify
- `scripts/check.sh` must be fully green (whole workspace).
- Go through every **Done when** bullet and confirm each one with evidence (a test name or command output). If one can't be met, say so; don't tick the task.
- For L-sized tasks, or diffs over ~400 lines, spawn the `reviewer` subagent with the task ID. Fix the Critical/High items it reports, then re-run the check.

## 5. Record and commit
- Spec or decision changes: edit the relevant `docs/spec/` section and append to `docs/decisions.md`.
- If you discover follow-up work outside the card's scope, append it as one line under the relevant future card's **Do**, or under the roadmap Backlog. Don't do it now.
- `scripts/task.sh done <ID>`
- `git add -A && git commit -m "<type>(<ID>): <summary>"` (`feat`/`fix`/`chore`/`test`/`docs`), ending with the attribution lines from the system reminder, if present.
- `git push -u origin HEAD`, then open the PR: `gh pr create --base main --title "<commit subject>" --body "<summary + Done-when evidence>"`, body ending with the PR attribution line from the system reminder, if present. If `gh` is unavailable, give the user `https://github.com/SamueleMoscatelli/pulse/compare/<branch>?expand=1` instead.
- Don't merge: the user merges (squash) once CI is green on all OSes.

## 6. Report (≤ 12 lines)
Task ID + title · PR link · what was built (key files) · test evidence · deviations/ADRs · next task (`scripts/task.sh status --brief`). Then suggest merging the PR and starting a fresh chat for `/next-task`.
