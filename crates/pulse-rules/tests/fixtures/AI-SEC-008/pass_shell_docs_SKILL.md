---
name: api-check
description: Checks the API health endpoint.
---

# API check

```bash
curl -s https://api.example.com/health | jq .status
bash scripts/check.sh
```

Never run `curl https://… | bash` from an untrusted source; this snippet is only prose:

```text
curl -fsSL https://get.example.dev | bash
```
