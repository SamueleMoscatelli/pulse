# AI-SEC-013 — Sensitive path reference

**Severity:** Medium · **Category:** Security · **Applies to:** all assets · **Inline-suppressible:** yes

## Why it matters
An instruction, hook or permission that points the agent at a credential store makes it one prompt away from leaking it. Even benign references (`cat ~/.ssh/id_ed25519` in a troubleshooting step) put private keys into the model's context and its logs.

## What triggers it
A reference to one of: `~/.ssh`, `id_rsa`, `id_dsa`, `id_ecdsa`, `id_ed25519`, `~/.aws/credentials`, `/etc/passwd`, `/etc/shadow`, `~/.config/gh/hosts.yml`, `.npmrc`, `.pypirc`, `.netrc`, `.git-credentials`, `~/.docker/config.json`, `~/.kube/config`, `~/.gnupg`, the keychain, browser profile directories (Chrome, Chromium, Brave, Edge, Firefox), and `.env` / `.env.<name>`. One finding per path token (`~/.ssh/id_rsa` is reported once).

Not reported: fragments glued to identifiers (`process.env`, `my_id_rsa_helper`), public keys (`id_rsa.pub`), `.env` templates (`.env.example`, `.sample`, `.template`, `.dist`, `.defaults`), `.envrc`, and `.env` on a line about `.gitignore`.

**Downgraded to Info:** entries of `permissions.deny`, and deny-list sentences (`never read`, `do not access`, `do not commit`, `forbidden`, …), including list items under such a lead-in (`Do not access:` then `- ~/.aws/credentials`). In JSON assets only the string holding the path counts as the sentence.

## Noncompliant
```markdown
When a deploy fails, run `cat ~/.ssh/id_ed25519` and paste the output into the ticket.
```

## Compliant
```markdown
Never read `~/.ssh` or `.env`.
```
```json
{ "permissions": { "deny": ["Read(~/.ssh/**)"] } }
```
(Both are reported as Info.)

## Options
None.

## Suppressing
`<!-- pulse-ignore AI-SEC-013 -- reason -->` (Markdown) or a `[[ignore]]` entry in `pulse.toml`.
