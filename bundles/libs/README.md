# OmniDoc library bundle

This directory contains the Pandoc filters, stylesheets, templates, reference
documents, CSL files, images, and TeX packages shipped as OmniDoc's library
bundle. It is developed and released from the OmniDoc monorepo.

## Compatibility

The machine-readable compatibility and payload contract is stored in
`manifest.toml`. The bundle version is always identical to the OmniDoc product
version. Version 1.6.0 targets OmniDoc 1.6.0 and Pandoc 3.x.

## Verify a checkout

From the OmniDoc monorepo root:

```bash
python3 bundles/libs/scripts/verify_manifest.py
bundles/libs/scripts/smoke-test.sh
```

Inside an extracted library source tree, use the corresponding `scripts/...`
paths without the `bundles/libs/` prefix.

LaTeX themes use Pandoc's own current default template rather than a vendored
copy. Run `pandoc -D latex` to inspect that template; the smoke test checks its
required extension hooks and compiles the engineering theme against it. Keep
custom presentation in theme `.sty` files and header/include hooks. The legacy
`pantext*.latex` files remain only for projects that explicitly selected them
and are not used by the engineering theme.

After intentionally changing a payload resource, regenerate checksums and
review the resulting diff:

```bash
python3 bundles/libs/scripts/verify_manifest.py --write
python3 bundles/libs/scripts/verify_manifest.py
```

Release archives should contain `manifest.toml`, `checksums.sha256`, and the
payload directories without modification. Consumers must verify the checksum
file before installing or updating the library bundle.

Build the deterministic release archive locally with:

```bash
bundles/libs/scripts/package-release.sh dist
OMNIDOC_RELEASE_TAG=v1.6.0 bundles/libs/scripts/package-release.sh dist
```

The command verifies all payload checksums, checks an optional tag against the
manifest version, creates `omnidoc-libs-v<version>.tar.gz`, writes its external
SHA-256 file, extracts the archive, and verifies the packaged payload again.
CI builds the archive twice and requires byte-for-byte identical output. The
matching OmniDoc `v<version>` tag publishes the library archive alongside the
platform-specific OmniDoc binaries in one GitHub release.

## Lua filter dependency protocol

For every active Lua filter, OmniDoc passes a metadata field named from the
normalized filter basename:

```text
omnidoc-depfile-<filter-stem>=/absolute/path/to/<filter-stem>.d
```

For example, `filters/custom-reader.lua` receives
`omnidoc-depfile-custom-reader` and should write:

```text
# omnidoc-depfile-v1
/absolute/path/to/an-actually-read-resource.json
```

Dependencies may be absolute or project-relative. OmniDoc consumes depfiles
only for filters active in the current output policy, content-hashes external
resources, and ignores malformed or stale depfiles. The include filters accept
this generic key while retaining their legacy metadata keys for compatibility.
