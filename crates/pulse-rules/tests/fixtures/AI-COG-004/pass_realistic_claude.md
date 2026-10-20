# Acme API

Node 20 service exposing the billing API. Postgres 15, Redis for queues.

## Commands
- Install: `pnpm install`
- Test: `pnpm test` (Vitest); run one file with `pnpm test src/invoice.test.ts`.
- Lint and format: `pnpm lint --fix` before every commit.

## Conventions
- Use TypeScript strict mode; no `any` outside `src/legacy/`.
- Keep functions under 40 lines and modules under 300.
- Name files in kebab-case, e.g. `invoice-service.ts`.
- Return errors as `Result<T, AppError>`; never throw from handlers.
- Never commit `.env` files. Secrets live in 1Password.
- Write a test for every bug fix, in `tests/regressions/`.

## Architecture
Handlers in `src/http/` call services in `src/services/`, which own all SQL.
Migrations live in `db/migrations/` and run with `pnpm db:migrate`.
