# OmniDoc plugin examples

The bundle includes practical plugin examples under `plugin-examples/`, but
they are intentionally not part of automatic plugin discovery. A hook can run
code during lint or build, so an example becomes active only after a user
explicitly installs it into a project.

```bash
omnidoc plugin examples
omnidoc plugin add quality-gate ./docs --dry-run
omnidoc plugin add quality-gate ./docs
omnidoc plugin validate ./docs
```

## Included examples

| Preset | Hook | Behavior |
|---|---|---|
| `quality-gate` | `lint_rule` | Reports TODO/FIXME markers and Markdown sources without an H1 |
| `asset-index` | `asset_provider` | Generates `generated/asset-index.md` from `assets/` and `images/` before dependency analysis |
| `build-journal` | `pre_build`, `post_build` | Appends structured lifecycle records under `.omnidoc-cache/` |

`plugin add` copies the complete preset to `plugins/<key>/` transactionally,
refuses an existing destination, supports side-effect-free `--dry-run`, and can
emit a stable JSON installation report. Installed copies are ordinary project
files: read their README and customize the scripts for local policy.

Every file belonging to a valid plugin with active hooks is recorded as a
resolved build resource. Changes to a manifest, script, or supporting file are
therefore visible in dependency output, cache decisions, build reports, and
`omnidoc.lock` instead of being hidden behind a stale cached artifact.

## Portable hook commands

Hook arrays are executed directly, without a shell. Besides the existing
environment variables, command arguments may use these placeholders:

- `{plugin_dir}`: absolute directory containing the plugin manifest;
- `{project_dir}`: absolute project root;
- `{output}`: active output format for build hooks;
- `{target}`: active artifact target name;
- `{python}` as the command: resolve `OMNIDOC_PYTHON`, `python3`, `python`, or
  the Windows `py -3` launcher.

Example:

```toml
[hooks]
lint_rule = ["{python}", "{plugin_dir}/scripts/lint.py"]
post_build = ["tool", "--project", "{project_dir}", "--format", "{output}"]
```

Processes also receive `OMNIDOC_PROJECT_DIR`, `OMNIDOC_PLUGIN_DIR`,
`OMNIDOC_PLUGIN_KEY`, `OMNIDOC_PLUGIN_MANIFEST_VERSION`, `OMNIDOC_HOOK`,
`OMNIDOC_OUTPUT`, and `OMNIDOC_TARGET`.
