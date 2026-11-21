# Local development

## Security review checklist

- **Email verification / password reset tokens** — single-use, expire within an hour, and are stored hashed.
- **Email verification / password reset tokens** — follow the [OWASP cheat sheet](https://cheatsheetseries.owasp.org/cheatsheets/Forgot_Password_Cheat_Sheet.html).
- **Session cookies** — `HttpOnly`, `Secure` and `SameSite=Lax`.

## Slack app setup

1. **OAuth & Permissions → Redirect URLs**: add `https://<tunnel-host>/slack/oauth/callback`, then add the scopes `chat:write` (post messages as the bot) and `users:read`, and copy the bot token into `.env` as `SLACK_BOT_TOKEN`.
2. **Event Subscriptions → Request URL**: `https://<tunnel-host>/slack/events`.

## Evaluations

Read [offline evaluation reporting](../evals/reporting.md), then post the scores and the conversation IDs in the PR description.

## Form fields

- `SourceFieldInputConfig` — basic input field types: `text`, `email`, `password`, `number`.

## Account recovery

- **Password reset tokens** — bound to the account and sent only to the verified address.
- Email the reset token to the user, then copy the session token into `.env.local`.
