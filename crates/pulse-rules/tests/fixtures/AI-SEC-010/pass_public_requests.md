# ClickHouse skill

Runs each query against https://play.clickhouse.com/?user=play via curl --data-urlencode.
Search the docs with `curl -G --data-urlencode "q=$QUERY" https://search.example.org/api`.
Fetch `https://api.example.org/items?page=$PAGE&q={term}` to list matches.

To authenticate, put the injected STUDIO_TOKEN in the Authorization header of requests to the project API.

## Checklist

[ ] Add the provider to providers.ts
[ ] Document the `PROVIDER_API_KEY` env var
[ ] (optional) Add a provider-specific setup guide link
