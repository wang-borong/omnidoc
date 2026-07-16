# Changelog

## 1.3.1

- Fixed macOS and Windows real-document smoke configuration.
- Added output-scoped `doctor --output` diagnostics.
- Normalized dependency and CSS paths across Windows and macOS.
- Disabled Git line-ending conversion for checksum-verified library clones.
- Vendored OpenSSL for portable release builds.
- Fixed release publishing from artifact-only jobs.
- Bound OmniDoc 1.3.1 to omnidoc-libs 1.0.1.

## 1.3.0

### Build reproducibility

- Replaced persistent input hashes with BLAKE3 and introduced lock schema v4.
- Locked resources independently for each output, including shared CSS, Lua
  filters, theme manifests, templates, fonts, toolchain versions, system LaTeX
  packages, and TeX recorder inputs.
- Added cache schema v6 with component-level invalidation reasons.
- Added atomic lock/cache/report writes and project-level writer exclusion.

### Themes and libraries

- Added versioned theme bundles with format-specific CSS/templates, Lua filters,
  LaTeX packages, metadata defaults, font requirements, and compatibility
  profiles.
- Added verified, transactional omnidoc-libs install/update/status/verify flows,
  revision pinning, release archives, manifests, and checksums.

### Output quality

- Added a Readium EPUB compatibility profile, EPUBCheck CI, MathML leakage
  checks, repeated-heading ID fixes, and resource validation.
- Added Golden Book HTML/EPUB integration tests and a rendered Golden PDF visual
  contract with CJK font and LaTeX package checks.
- Unified format-specific Pandoc policy and command construction.

### Safety and diagnostics

- Fixed project-root inference in source diagnostics.
- Added structured build reports, cache explanations, `doctor --strict`, and
  release/package smoke tests.
- Made the formatter block/token aware, conservative and idempotent; added
  atomic writes, byte-format preservation, `fmt --check`, and `fmt --diff`.
- Added plugin manifest schema version 1 and OmniDoc compatibility ranges.

### Compatibility

- Lock and cache files from older versions must be regenerated.
- OmniDoc 1.3.0 is bound to omnidoc-libs 1.0.0.
