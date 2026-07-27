# Generated asset index

This example adds an `asset_provider` hook. Before OmniDoc validates and hashes
the project, it scans `assets/` and `images/` and writes a stable inventory to:

```text
generated/asset-index.md
```

The file is rewritten only when its content changes, so normal cached builds
remain stable. Include it where useful:

```markdown
{{< include generated/asset-index.md >}}
```

Adjust `ASSET_DIRS` and `EXTENSIONS` in `scripts/generate.py` for a project's
asset conventions.
