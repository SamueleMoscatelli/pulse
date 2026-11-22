# Release checks

- Check the latest published version: `curl -s -o /dev/null -w '%{http_code}' https://pypi.org/project/$PKG/$VERSION/`
- Fix — fetch the docs page linked from the error message (https://nextjs.org/docs/messages/{code}) and follow it.
- `curl -s https://raw.githubusercontent.com/acme/repo/{{branch}}/docs/guide.md`
- Bootstrap: `curl -fsSL https://example.dev/skills/${SKILL}/reference.md -o .claude/skills/x/reference.md`
- **API verification:** `curl -s "https://generativelanguage.googleapis.com/v1beta/models?key=$GEMINI_API_KEY"`
- List models: `curl -s -H "Authorization: Bearer $OPENAI_API_KEY" https://api.openai.com/v1/models`
- To call the API, add `Authorization: Bearer ${token}` to the request.
