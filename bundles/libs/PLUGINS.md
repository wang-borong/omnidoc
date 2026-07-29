# OmniDoc plugin packages

OmniDoc plugins are versioned extension packages containing Pandoc Lua filters
and/or explicit `pandoc lua` commands. OmniDoc does not embed a Lua runtime and
does not support lifecycle hooks such as `pre_build`, `post_build`,
`lint_rule`, or `asset_provider`.

Installed plugins are inert by default. An automatic filter runs only after
the exact package payload has been trusted on the current machine and enabled
in a project's `.omnidoc.toml`.

```bash
omnidoc plugin install-example quality-gate --project ./docs
omnidoc plugin trust omnidoc/quality-gate@=1.0.0 --project ./docs
omnidoc plugin enable omnidoc/quality-gate@=1.0.0 ./docs
omnidoc plugin validate omnidoc/quality-gate@=1.0.0 --project ./docs --check-lua
```

## Included examples

| Preset | Type | Behavior |
|---|---|---|
| `quality-gate` | automatic filter | Warns about TODO/FIXME markers and missing H1 headings; optional strict failure metadata |
| `metadata-stamp` | automatic filter | Adds deterministic generator/plugin metadata to the Pandoc document |
| `word-count` | explicit command | Counts rendered words and characters only when invoked by the user |

Run the command-only example with:

```bash
omnidoc plugin install-example word-count --project ./docs
omnidoc plugin trust omnidoc/word-count@=1.0.0 --project ./docs
omnidoc plugin run omnidoc/word-count word-count --project ./docs -- main.md
```

## Package manifest

Every package uses `omnidoc-package.toml` and manifest version 2:

```toml
manifest_version = 2
kind = "plugin"
id = "acme/document-tools"
name = "Document Tools"
version = "1.2.0"
compatible_omnidoc = ">=1.8,<2"
compatible_pandoc = ">=3,<4"

[plugin]
api_version = 1

[[plugin.filters]]
script = "filters/normalize.lua"
formats = ["pdf", "html", "epub", "docx"]
order = 500
# Optional dynamic-dependency channel. The key must be globally unique among
# all enabled plugins.
dependency_key = "acme-document-tools-inputs"

[[plugin.commands]]
name = "word-count"
script = "commands/word-count.lua"
description = "Count rendered words"
```

Filter order is deterministic: numeric `order`, plugin ID, then script path.
Core OmniDoc filters always run before extension filters. A filter with no
`formats` list applies to every supported output.

`compatible_pandoc` is required. OmniDoc checks the configured Pandoc version
before a plugin can be enabled, validated, loaded as a filter, or run as an
explicit command. Project `[plugins].enabled` entries must use exact
`id@=version` pins; ranges and unversioned IDs are rejected.

`plugin validate --check-lua` compiles each script with `loadfile` inside
`pandoc lua -e`. The script path is supplied through an environment variable,
not as Pandoc's executable script argument, so top-level plugin code is not run
during validation.

Filters that read files outside the Pandoc document declare `dependency_key`.
For the example above OmniDoc passes
`omnidoc-plugin-depfile-acme-document-tools-inputs` with an absolute path to
`.omnidoc-cache/plugin-acme-document-tools-inputs.d`. The filter writes
`# omnidoc-depfile-v1` followed by one absolute or project-relative dependency
per line. Keys are explicit instead of being derived from common names such as
`main.lua`, so two independently authored plugins cannot silently share one
depfile; duplicate active keys are rejected. OmniDoc clears the active depfile
before Pandoc runs, so the filter must write a fresh header even when it has no
dependency lines.

Explicit commands receive `OMNIDOC_PROJECT_DIR`, `OMNIDOC_PLUGIN_DIR`,
`OMNIDOC_PLUGIN_ID`, and `OMNIDOC_PLUGIN_VERSION`. They still require trust,
but they do not need to be enabled because invocation itself is explicit.

Package stores are locked while plugin payloads are read or changed. Builds,
validation, trust updates, and explicit commands therefore see one complete
payload digest. An exact uninstall remains available for a damaged installed
package even if its manifest can no longer be parsed.

The installed layout is
`<store>/plugins/<ID segments>/<VERSION>/omnidoc-package.toml`. A package has
exactly one root manifest. IDs and versions are restricted to lower-case,
cross-platform-safe path components, and exact pins compare SemVer build
metadata as part of package identity. Replacement uses a recoverable
transaction; an unexpected promoted digest leaves both the destination and
backup untouched for inspection instead of guessing which payload to keep.
