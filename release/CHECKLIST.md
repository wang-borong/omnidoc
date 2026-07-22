# OmniDoc Release Checklist

OmniDoc and `bundles/libs` use one version, one tag, and one GitHub release.
The release publishes platform-specific binaries plus the verified
`omnidoc-libs-v<version>.tar.gz` sidecar bundle.

## 1. Prepare the release

1. Set the version once; the command updates `Cargo.toml`, `Cargo.lock`, the
   bundle manifest/compatibility, and the embedded release contract:

   ```bash
   python3 scripts/set-version.py 1.6.0
   ```

2. Regenerate and verify the payload checksums:

   ```bash
   python3 bundles/libs/scripts/verify_manifest.py --write
   python3 bundles/libs/scripts/verify_manifest.py
   python3 scripts/check-library-contract.py
   ```

## 2. Verify the release candidate

```bash
cargo fmt -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps --document-private-items --all-features
bundles/libs/scripts/smoke-test.sh
scripts/check-golden-book.sh
scripts/check-golden-pdf.sh
```

Build the deterministic sidecar twice and compare it:

```bash
OMNIDOC_RELEASE_TAG=v1.6.0 bundles/libs/scripts/package-release.sh /tmp/omnidoc-libs-a
OMNIDOC_RELEASE_TAG=v1.6.0 bundles/libs/scripts/package-release.sh /tmp/omnidoc-libs-b
cmp /tmp/omnidoc-libs-a/*.tar.gz /tmp/omnidoc-libs-b/*.tar.gz
cmp /tmp/omnidoc-libs-a/*.sha256 /tmp/omnidoc-libs-b/*.sha256
```

CI installs the locally built sidecar through `OMNIDOC_LIBS_ARCHIVE` and
`OMNIDOC_LIBS_CHECKSUM` before the GitHub release exists, exercising the same
checksum, extraction, manifest, compatibility, and transactional replacement
path used for normal downloads.

## 3. Reader acceptance matrix

Open the Golden Book EPUB and record the application version and result for:

- Thorium/Readium: navigation, MathML, admonitions, cover and CSS.
- Apple Books: CJK fonts, inline/block math, table overflow and links.
- Calibre Viewer: navigation, images, code blocks and metadata.
- Kindle Previewer: conversion warnings, cover, headings and fallback math.

Any reader-specific exception must be represented by a named compatibility
profile or documented release note rather than an untracked CSS patch.

## 4. Publish

1. Commit, create, and push the single product tag, for example `v1.6.0`.
2. Require quality, library bundle, Golden Book, Golden PDF, portable document
   smoke, package, and installed-release smoke jobs to pass.
3. Confirm the GitHub release contains every binary package plus
   `omnidoc-libs-v1.6.0.tar.gz` and its `.sha256` file.
4. Download every archive and run its packaged-binary smoke test.
5. Publish release notes describing lock/cache schema changes and bundle changes.
