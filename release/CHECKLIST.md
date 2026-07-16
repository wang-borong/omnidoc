# OmniDoc Release Checklist

Release omnidoc-libs before the matching OmniDoc version. Do not create the
OmniDoc tag while any release-contract URL returns 404.

## 1. Release omnidoc-libs

1. Confirm `manifest.toml` version and compatibility range.
2. Run:

   ```bash
   python3 scripts/verify_manifest.py
   OMNIDOC_RELEASE_TAG=v1.0.1 scripts/package-release.sh /tmp/omnidoc-libs-release
   scripts/smoke-test.sh
   ```

3. Commit, create and push the signed `v1.0.1` tag.
4. Wait for the omnidoc-libs release workflow.
5. Download the archive and checksum from a clean environment and verify
   `sha256sum --check`.

## 2. Verify the OmniDoc release candidate

Run from the OmniDoc repository:

```bash
python3 scripts/check-library-contract.py ../omnidoc-libs
cargo fmt -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps --document-private-items --all-features
OMNIDOC_LIBS=../omnidoc-libs scripts/check-golden-book.sh
OMNIDOC_LIBS=../omnidoc-libs scripts/check-golden-pdf.sh
```

Confirm both URLs in `omnidoc-libs.toml` return success, then exercise the
packaged release flow:

```bash
curl --fail --location --head \
  https://github.com/wang-borong/omnidoc-libs/releases/download/v1.0.1/omnidoc-libs-v1.0.1.tar.gz
curl --fail --location --head \
  https://github.com/wang-borong/omnidoc-libs/releases/download/v1.0.1/omnidoc-libs-v1.0.1.tar.gz.sha256
OMNIDOC_BIN=/path/to/extracted/omnidoc scripts/check-release-install.sh
```

## 3. Reader acceptance matrix

Open the Golden Book EPUB and record the application version and result for:

- Thorium/Readium: navigation, MathML, admonitions, cover and CSS.
- Apple Books: CJK fonts, inline/block math, table overflow and links.
- Calibre Viewer: navigation, images, code blocks and metadata.
- Kindle Previewer: conversion warnings, cover, headings and fallback math.

Any reader-specific exception must be represented by a named compatibility
profile or documented release note rather than an untracked CSS patch.

## 4. Publish OmniDoc

1. Confirm `Cargo.toml`, `Cargo.lock`, and `release/omnidoc-libs.toml` agree.
2. Commit, create and push the signed `v1.3.2` tag.
3. Require quality, Golden Book, Golden PDF, portable document smoke, package,
   and installed-release smoke jobs to pass.
4. Download every archive and run its packaged-binary smoke test.
5. Publish release notes describing lock/cache schema changes and the required
   omnidoc-libs version.
