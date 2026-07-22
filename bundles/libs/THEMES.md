# OmniDoc shared themes

Theme bundles have machine-readable manifests under `themes/`. Inspect and
validate the installed contracts with:

```bash
omnidoc theme list
omnidoc theme inspect engineering-book
omnidoc theme validate engineering-book
```

## Engineering textbook

The engineering textbook theme provides matching PDF, HTML, EPUB, and PPTX styles.

Select the bundle once in `.omnidoc.toml`:

```toml
[theme]
name = "engineering-book"
version = "1"
compatibility = "readium"
```

OmniDoc resolves the matching HTML/EPUB CSS, Lua filters, PDF header, and PPTX
reference deck from the bundle manifest. PDF generation intentionally uses
Pandoc's built-in,
version-matched LaTeX template; the engineering design is injected through the
header and `omni-engineering-book.sty`. HTML and EPUB use the same responsive typography,
semantic-block styling, MathML layout, tables, code blocks, figures, and dark mode. Desktop
HTML is centered at a maximum reading width of 56rem.

Portable styling is split into focused modules under `pandoc/css/modules/`:
design tokens, semantic blocks, code, tables, math, and figures. The theme
manifest lists them in cascade order, so OmniDoc passes every stylesheet to
Pandoc and records every module in the dependency graph. EPUB output therefore
does not depend on reader-specific CSS `@import` behavior.

Presentation projects use the themed reference deck automatically:

```bash
omnidoc build --to pptx
```

Set the deck title in document metadata and use level-2 headings for individual
slides (`--slide-level=2`). Level-1 headings remain available as section divider
slides. The reference deck supplies the engineering palette, CJK-aware fonts,
slide layouts, and a consistent accent bar while keeping text and diagrams
editable in PowerPoint-compatible applications.

Optional PDF cover fields can still be set from document metadata:

```yaml
header-includes:
  - \renewcommand{\OmniBookSubtitle}{A reusable subtitle}
  - \renewcommand{\OmniBookImprint}{2026}
```

The package is installed as `texmf/tex/common/omni-engineering-book.sty` and
loaded automatically through `pandoc/headers/engineering-book.tex` when the
project selects `[theme] name = "engineering-book"`. It provides the cover,
engineering color palette, CJK typography, headings, headers and footers,
quote boxes, and alternating table rows. Semantic fenced blocks are provided
by `omni-blocks`, split into core tokens and an admonition component. The
subtitle and imprint commands are
optional; without them, the subtitle is omitted and the document date is used.
The package loads its required XeLaTeX/CJK and semantic-block support itself, so a
theme selection is sufficient to compile its declared components.
Its manifest also declares the system LaTeX packages resolved through
`kpsewhich`; OmniDoc can validate their presence and locks the resolved `.sty`
version/content identities for PDF builds.

The public fenced syntax is documented centrally in [`BLOCKS.md`](BLOCKS.md).

This split is deliberate: do not copy Pandoc's full `default.latex` into the
theme. Inspect the active upstream contract with `pandoc -D latex`, keep visual
customization in the `.sty` package, and use Pandoc's supported
`header-includes`, `include-before`, and `include-after` hooks for project-level
extensions. `scripts/check-pandoc-latex-template.sh` verifies those upstream
hooks and compiles a representative themed document against the installed
Pandoc version.

For source images that remain SVG in HTML and EPUB, place a pre-rendered PDF
with the same basename next to the SVG (for example, `diagram.svg` and
`diagram.pdf`). PDF/LaTeX builds select that sibling deterministically without
requiring shell escape or Inkscape, and OmniDoc records both assets in the
target dependency graph.
