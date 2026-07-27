# Changelog

## Unreleased

- Added `corporate-docs`, `classic-book`, `clean-document`, and
  `modern-slides` theme bundles with distinct HTML/EPUB/PDF styling, themed
  DOCX reference documents, a modern PPTX reference deck, categorized theme
  discovery, and safe `theme apply|use` project selection.
- Added opt-in `quality-gate`, `asset-index`, and `build-journal` plugin
  examples plus `plugin examples` and transactional `plugin add` workflows;
  hook arguments now support safe project/plugin/output placeholders and a
  portable `{python}` launcher, while active plugin files now participate in
  dependency, cache, report, and lock identities.
- Added workflow-oriented `check`, `convert`, and `template` command groups
  while retaining the previous flat commands for script compatibility, and
  reorganized top-level help around common project workflows.
- Added discoverable `lib install|update|status|verify` and
  `plugin list|validate` command groups while retaining the previous action
  flags and flat plugin form for script compatibility; grouped plugin JSON
  failures now preserve the standard machine-readable error envelope.
- Made `new` and `init` titles optional, added direct `--type`, filtered
  `--format`, and non-interactive `--defaults` creation paths, plus actionable
  next-step output and positional shell completion generation.
- Added `new --dry-run`, `--json`, and `--no-commit` with exact creation action
  reports and structured failures; new repositories now receive one
  content-bearing initial commit instead of an empty commit followed by a
  second project commit.
- Made project creation fail safely before touching the target when an
  interactive template cannot be selected, stopped cancellation from deleting
  existing init directories, and fixed relative init/open/clean/update path
  handling.
- Carried external template language and entry-file metadata through project
  setup and configuration, added JSON template discovery/validation with safe
  relative-path enforcement, and made initial Git commits work on machines
  without global Git identity settings.
- Added `status [--json]` for resolved project, entry, output, target, and
  artifact discovery; made `open` honor configured formats/targets and support
  `--to` plus the composable `--print-path` mode across platform viewers.
- Added stable JSON clean reports and `clean --dry-run`, aligned regular clean
  behavior with the documented build-directory contract, preserved unrelated
  project PDFs, and rejected absolute or project-escaping output directories
  before deletion.
- Unified project-root discovery across build, watch, publish, validation,
  dependency, lock, plugin, status, open, clean, and update workflows, so the
  same commands and explicit paths work consistently from nested directories.
- Added grouped `config init`, `config show`, and `config get` workflows with
  selectable merged/global/project scopes, stable JSON, source provenance, and
  no implicit config-file creation from read-only commands; legacy flat config
  options remain accepted but are hidden from primary help.
- Added typed `config set` and `config unset` workflows for project/global
  files with scope-aware key validation, dry-run/diff previews, stable JSON,
  semantic no-op detection, comment-preserving TOML edits, atomic writes, and
  complete first-run defaults when bootstrapping global configuration.
- Fixed `paths.*` precedence so project configuration now overrides global
  values as documented while continuing to inherit unspecified global paths.
- Added `update --dry-run`, `--diff`, `--no-commit`, and stable JSON action and
  repository-status reports; previews now contain only real changes and can
  include unified managed-file diffs.
- Prevented `update` and `init` from mixing pre-existing repository changes into
  automatic commits, added `init --no-commit`, made managed-file writes atomic,
  skipped no-op update commits, staged tracked moves/deletions correctly, and
  retained support for creating the first commit in an unborn repository.
- Added `init --dry-run`, `--diff`, and stable JSON reports covering inferred
  metadata, Git readiness, exact file/directory/source-move actions, and unified
  managed-file diffs without modifying the existing directory.
- Kept update project locking and collision-safe mixed Markdown/LaTeX source
  moves, aborting before writes when a destination already exists.
- Prevented `watch` rebuild loops by ignoring configured output directories,
  `dist`, generated figure directories, caches, and exact root-level generated
  artifacts while continuing to watch unrelated source assets; `watch --once`
  now returns a failing build's non-zero status instead of reporting success.

## 1.7.0 - 2026-07-26

- Bundled the official Tectonic 0.16.9 engine in platform release archives and
  made it the automatic first choice for managed Markdown PDF builds, with
  XeLaTeX fallback and a XeLaTeX/latexmk-first policy for raw LaTeX projects.
- Added Tectonic bundle, offline-cache, explicit shell-escape, recursive search
  path, CJK/OmniDoc texmf compatibility, and Makefile dependency recording.
- Made project-level `[tools]` settings override global tools and stopped the
  CLI's implicit `latexmk` value from overriding `build.latex_backend`.
- Preserved document front-matter title and author instead of replacing them
  with the output target and the global template-author default.
- Added the generic Simplified Chinese Pandoc translation map required by
  `zh-CN`, eliminating missing structural-term translations in CJK outputs.
- Unified project-type inference across build, doctor, cache, and lock flows;
  fixed relative single-file conversions and tracked external local Tectonic
  bundles by content.
- Declared the bundled Linux engine's dynamic runtime dependencies in package
  metadata and added packaged-binary linkage checks.
- Added engine kind/origin and Tectonic bundle policy to doctor, reports,
  locks, and cache fingerprints without requiring kpsewhich in Tectonic mode.
- Removed a bundled `listings.sty` shadowing bug and made the engineering-book
  table styling compatible with older Tectonic web bundles.
- Added Tectonic Golden PDF gates for both Markdown and native LaTeX projects,
  verified CJK/font/emoji/table rendering and dependency invalidation, and made
  TeX Live optional in Arch and Debian package metadata.

## 1.6.1

- Normalized bundled manifest paths into native path components so theme CSS,
  Lua filters, LaTeX headers, templates, and reference documents work reliably
  in Windows builds and packaged releases.

## 1.6.0

- Moved the OmniDoc library payload into `bundles/libs`, so Rust, Lua, CSS,
  TeX, themes, tests, and release metadata now change atomically in one repo.
- Unified the OmniDoc and library bundle version at 1.6.0 and publish both the
  platform binaries and `omnidoc-libs-v1.6.0` sidecar from one product tag.
- Removed Git clone, branch, tag, and commit-based library installation; the
  CLI now installs and updates only verified release-bound archives.
- Preserved OmniDoc-managed LaTeX headers when projects add their own
  `--include-in-header` options, including semantic block packages for PDF.

## 1.5.1

- Bound release builds to omnidoc-libs 1.2.1, including refreshed fenced
  diagram outputs when a stable figure identifier is reused.
- Derived the release library revision from `release/omnidoc-libs.toml` in CI
  instead of maintaining a second hard-coded version.

## 1.5.0

- Added first-class PPTX presentation builds with release-bound engineering
  reference slides.
- Added fenced `bitfield` diagram blocks that render as SVG for HTML, EPUB,
  DOCX, and PPTX, and as PDF for PDF/LaTeX output.
- Passed the running OmniDoc executable to Pandoc filters so native diagram
  renderers use the exact installed version.
- Fixed project discovery for `.omnidoc.toml` projects and prevented LaTeX
  recorder environment variables from misrouting nested OmniDoc invocations.

## 1.4.0

- Added first-class `[pandoc] toc = true` support so HTML and EPUB tables of
  contents are generated instead of rendering an empty navigation block.
- Made `latexmk` rebuild after a cached failed invocation and preserved the
  useful LaTeX log summary before cleanup.
- Reduced LaTeX diagnostic noise from package info messages.
- Preserved all configured output targets when refreshing build locks.
- Resolved percent-encoded and angle-wrapped Markdown resource paths.
- Restored use of Pandoc's installed LaTeX template with theme overlays.
- Added reliable color emoji rendering for LaTeX/PDF output through the
  release-bound omnidoc-libs 1.1.1 bundle.

## 1.3.3

- Fixed installed-release smoke archive discovery when GitHub artifact
  directories use the same name as the contained archive.

## 1.3.2

- Restored isolated XDG config/data paths on macOS and Windows.
- Fixed checksum fixtures across repeated Windows Git updates.
- Moved Intel macOS CI jobs to the supported `macos-15-intel` runner.

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
