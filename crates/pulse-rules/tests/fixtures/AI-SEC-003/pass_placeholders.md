# Setup

Export your keys before running the agent:

```bash
export OPENAI_API_KEY=sk-your-key-here-replace-me
export ANTHROPIC_API_KEY="<your anthropic key>"
export AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE
export GITHUB_TOKEN=ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
```

In CI, read them from the environment: `api_key = os.environ["OPENAI_API_KEY"]`
or `const token = process.env.API_TOKEN;`. Never commit `password: changeme-in-prod`.
