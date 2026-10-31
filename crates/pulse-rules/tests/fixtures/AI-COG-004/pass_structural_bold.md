# Supply chain

Generate the manifest after code review.

**Output (`INTEGRITY.json`):**

```json
{ "file_count": 3 }
```

**Output on tampered plugin:**

```text
MISMATCH skills/a/SKILL.md
```

**WARNING**: run the verifier before every deploy.

**Step 2:** Commit the manifest with the PR.

| Practice | Why |
| -------- | --- |
| **Generate manifest after code review** | Reviewed code matches production |
| **Include manifest in the PR** | Reviewers can verify the hashes |
| **Verify in CI before deploy** | Catches post-review changes |
| **Pin all dependency versions** | Unpinned deps change on install |
