# Security policy

## Supported versions

Security fixes go into the latest release. Pulse follows semantic versioning, so upgrading
within a major version is always safe.

## Reporting a vulnerability

Please **do not open a public issue**. Report it privately through
[GitHub private vulnerability reporting](https://github.com/SamueleMoscatelli/pulse/security/advisories/new),
with the Pulse version, the steps or asset that reproduce it, and the impact you see.

You will get an acknowledgement within a week. Once a fix is released, the advisory is published
and you are credited, unless you prefer not to be.

## Scope

Pulse reads untrusted files in CI and pre-commit hooks, so these count as vulnerabilities:

- a crafted asset or `pulse.toml` that makes Pulse crash, hang, or use unbounded memory or time
- Pulse reading or writing files outside the scan root, or following a symlink it should not
- a secret that Pulse detects showing up unredacted in any report format
- a way to silence a rule that should not be suppressible inline (AI-SEC-001/002/003/008/012)
- problems in the release artifacts, the installers or the GitHub Action

A missed detection (an injection phrase or secret format Pulse does not know yet) is a rule
improvement, not a vulnerability: open a regular issue for it.
