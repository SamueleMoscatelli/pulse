# Local setup

1. Copy `.env.example` to get the variable names, then fill them in.
2. Add your local env file to `.gitignore` (`.env` must never be committed... see the gitignore section).
3. The app reads `process.env.PORT`; the deploy key is published as `id_rsa.pub`.
