# ADR 0001: Tectonic PDF engine policy

- Status: Accepted
- Date: 2026-07-25
- Owners: OmniDoc maintainers

## Context

OmniDoc PDF users previously needed a TeX Live installation even for managed
Markdown documents. The desired outcome is a working CJK/English PDF path in
the official OmniDoc package while retaining compatibility for advanced native
LaTeX projects.

Tectonic exposes both Rust crates and a command-line engine. Statically linking
its internal crates into OmniDoc would couple the release to Tectonic's native
build graph and less-stable library APIs on every supported platform. Shipping
the pinned upstream executable preserves a narrow process boundary, keeps
OmniDoc's Rust build small, and allows the engine to be replaced independently.

## Decision

1. Official OmniDoc archives ship the unmodified Tectonic 0.16.9 executable.
   Every upstream asset is selected explicitly and verified with a pinned
   SHA-256 digest by `scripts/fetch-tectonic.sh`.
2. Managed Markdown PDF builds resolve engines in this order:
   bundled Tectonic, Tectonic from `PATH`, then XeLaTeX.
3. Native LaTeX projects resolve XeLaTeX first and use Tectonic only when
   XeLaTeX is unavailable or the user explicitly selects Tectonic.
4. A bundled Tectonic file must successfully identify itself through
   `--version`; an unusable bundled executable is skipped instead of blocking
   a working `PATH` or XeLaTeX fallback.
5. Tectonic receives OmniDoc's local texmf, project TeX/BibTeX paths, configured
   search paths, bundle/offline policy, and explicit shell-escape policy.
   Shell escape remains disabled by default.
6. Tectonic Makefile rules are normalized into OmniDoc's dependency file so
   local indirect inputs participate in cache, report, and lock generation.

## Compatibility evidence

The Golden PDF gates validate the following with both searchable text and
embedded fonts where applicable:

- English, CJK, Unicode, OpenType fonts, `fontspec`, and `xeCJK`;
- mathematics, tables, listings, TikZ, tcolorbox, and emoji;
- automatic TeX reruns and BibTeX-style bibliography processing;
- Markdown/Pandoc and native LaTeX entry paths;
- recursive project and OmniDoc texmf lookup;
- indirect dependency invalidation, lock files, and PDF visual contracts.

The official x86_64 Linux GNU asset is retained because it passes the full
contract. Its current glibc floor is 2.39. The upstream static musl asset was
evaluated but rejected: it embeds ICU 71 without the required matching data and
fails the CJK Golden Book with `failed to create linebreak iterator, status=2`.

## Non-equivalence with XeLaTeX plus latexmk

Tectonic is not treated as a complete behavioral replacement for TeX Live:

- it does not read `latexmkrc`;
- OmniDoc does not orchestrate Biber through Tectonic;
- shell-escape support is unstable and project-specific;
- the default bundle requires network access on first use unless its cache is
  prewarmed or a complete local bundle is configured;
- document fonts remain operating-system resources;
- specialized TeX utilities and highly customized native projects may still
  require XeLaTeX, latexmk, and TeX Live.

For these reasons, Tectonic is the managed Markdown default but not the native
LaTeX default.

## Consequences and rollback

Release size grows by the engine binary and release CI must validate all pinned
assets. Linux package metadata declares the GNU asset's runtime libraries.
Users on an older Linux runtime can install a compatible Tectonic in `PATH` or
select XeLaTeX.

The policy is reversible without changing document formats: users can set
`tools.latex_engine = "xelatex"`, and maintainers can remove the packaged asset
while retaining the same resolver and Golden XeLaTeX gate.
