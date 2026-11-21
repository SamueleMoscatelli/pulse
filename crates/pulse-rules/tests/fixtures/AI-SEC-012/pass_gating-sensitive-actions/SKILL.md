---
name: gating-sensitive-actions
description: Use when adding an endpoint that deletes data or changes billing. Explains how to require sudo mode (a fresh re-authentication) before the action runs.
---

# Gating sensitive actions

Wrap the view in `@require_sudo_mode`.
