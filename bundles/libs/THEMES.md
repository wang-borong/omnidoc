# OmniDoc shared themes

Theme bundles have machine-readable manifests under `themes/`. Each manifest
binds the resources needed by the relevant writer instead of relying on a
single CSS file.

## Included themes

| Theme | Category | Recommended use | Styled outputs |
|---|---|---|---|
| `engineering-book` | book | Engineering textbooks, courses, training material | PDF/LaTeX, HTML, EPUB, DOCX, PPTX |
| `corporate-docs` | corporate | User manuals, developer guides, API docs, internal standards | PDF/LaTeX, HTML, EPUB, DOCX, PPTX |
| `classic-book` | book | General books, essays, histories, long-form reading | PDF/LaTeX, HTML, EPUB, DOCX |
| `clean-document` | document | Reports, specifications, proposals, meeting notes | PDF/LaTeX, HTML, EPUB, DOCX |
| `modern-slides` | presentation | Product briefings, technical talks, training decks, reviews | PPTX plus matching HTML/EPUB and PDF handouts |

Discover, inspect, validate, and select themes with:

```bash
omnidoc theme list
omnidoc theme inspect corporate-docs
omnidoc theme validate
omnidoc theme apply corporate-docs ./my-manual
# `theme use` is an alias of `theme apply`.
```

`theme apply --dry-run`, `--diff`, and `--json` use the same safe,
comment-preserving project configuration editor as `omnidoc config set`.

## Cross-format behavior

OmniDoc resolves the selected theme's resources for each build target:

- HTML and EPUB receive ordered token, layout, semantic-block, code, table,
  math, and figure stylesheets.
- PDF and LaTeX receive a small header plus a maintained `.sty` package while
  continuing to use Pandoc's version-matched default LaTeX template.
- DOCX receives a theme-specific reference document unless the project sets
  `pandoc.reference_doc` explicitly.
- PPTX receives a theme-specific reference deck unless the project sets
  `pandoc.pptx_reference_doc` or `pandoc.reference_doc` explicitly.
- Core/plugin Lua filters and every selected theme resource are recorded in the dependency graph,
  cache input, report, and lock file.

For installable manifest-v2 themes, an explicit `theme.outputs` list is
authoritative. Resources and generated token assets are applied only to those
writers; validation rejects resource/output mismatches. If `theme.outputs` is
omitted, OmniDoc infers it from the declared resources and semantic tokens. An
explicit child list replaces its inherited output set; an omitted child list
inherits and extends the parent's capabilities. OmniDoc validates the fully
resolved chain before applying it.

Installable themes use the canonical
`<store>/themes/<ID segments>/<VERSION>/omnidoc-package.toml` layout with one
root manifest. Package IDs, versions, archive paths, case collisions, and
symbolic links are checked for cross-platform safety. Replacement is
transactional and automatically recovered by the next mutating extension
command; digest conflicts preserve the backup for inspection.

For PDF builds, directories containing declared installable
`theme.resources.latex_packages` are searched child-first. Tectonic receives
`-Zsearch-path`; XeLaTeX, LuaLaTeX, and pdfLaTeX receive a prepended
`TEXINPUTS` while retaining the existing/default TeX search path.

All bundled themes use CJK-aware Noto font families. Run environment checks
when producing release artifacts:

```bash
omnidoc theme validate corporate-docs --check-fonts --check-latex
omnidoc check doctor --strict --output pdf ./my-manual
```

## Theme notes

### Corporate docs

`corporate-docs` uses compact sans-serif typography, a blue/teal enterprise
palette, strong heading hierarchy, wide readable pages, and reference styles
suited to manuals and technical guides. It is the best general default for a
company documentation portal that also publishes PDF or Word deliverables.
Its print and Word defaults use A4 pages.

### Classic book

`classic-book` uses a warmer paper palette, serif typography, narrower reading
measure, paragraph indentation, centered chapter hierarchy, and restrained
ornaments for long-form reading.
Its print and Word defaults use a compact A5 book page.

### Clean document

`clean-document` is deliberately neutral. It uses compact spacing, grayscale
headings, subtle rules, and conventional report typography without imposing a
strong brand.
Its print and Word defaults use A4 pages.

### Modern slides

`modern-slides` supplies a distinct high-contrast PPTX reference deck with
CJK-aware fonts, modern blue/teal accents, editable layouts, and matching
handout styles. Use level-2 headings for individual slides and level-1 headings
for section dividers:

```bash
omnidoc theme apply modern-slides
omnidoc build --to pptx
```

### Engineering book

`engineering-book` remains the dense teaching/technical-book profile. It
provides the circuit-inspired palette, semantic learning blocks, engineering
tables and code, a designed PDF cover, a DOCX reference document, and the
existing engineering reference deck.

Optional PDF cover fields can be set from document metadata:

```yaml
header-includes:
  - \renewcommand{\OmniBookSubtitle}{A reusable subtitle}
  - \renewcommand{\OmniBookImprint}{2026}
```

The public fenced-block syntax is documented centrally in
[`BLOCKS.md`](BLOCKS.md).

For source images that remain SVG in HTML and EPUB, place a pre-rendered PDF
with the same basename next to the SVG (for example, `diagram.svg` and
`diagram.pdf`). PDF/LaTeX builds select that sibling deterministically without
requiring shell escape or Inkscape, and OmniDoc records both assets in the
target dependency graph.
