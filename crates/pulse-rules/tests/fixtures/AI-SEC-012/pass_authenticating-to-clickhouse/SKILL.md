---
name: authenticating-to-clickhouse
description: Covers the token-aware default path for connecting to ClickHouse, how the CLI passes the access token to the driver, and when it falls back to a password. Use when a query fails with an authentication error.
---

# Authenticating to ClickHouse

Check `CLICKHOUSE_TOKEN` first, then the connection profile.
