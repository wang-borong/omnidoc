<p align="center">
  <img src="assets/omnidoc-icon.png" alt="OmniDoc icon" width="190">
</p>

<h1 align="center">OmniDoc</h1>

<p align="center">
  <strong>Write once. Publish everywhere.</strong><br>
  An extensible, reproducible publishing system for Markdown and LaTeX.
</p>

<p align="center">
  <a href="https://github.com/wang-borong/omnidoc/actions/workflows/CICD.yml"><img alt="CI" src="https://github.com/wang-borong/omnidoc/actions/workflows/CICD.yml/badge.svg"></a>
  <a href="https://github.com/wang-borong/omnidoc/releases"><img alt="Release" src="https://img.shields.io/github/v/release/wang-borong/omnidoc?display_name=tag"></a>
  <a href="LICENSE"><img alt="MIT License" src="https://img.shields.io/github/license/wang-borong/omnidoc"></a>
  <img alt="Platforms" src="https://img.shields.io/badge/platform-Linux%20%7C%20macOS%20%7C%20Windows-14b8a6">
</p>

<p align="center">
  <a href="#-quick-start">Quick start</a> ·
  <a href="#-why-omnidoc">Features</a> ·
  <a href="#-themes-plugins-and-templates">Extensions</a> ·
  <a href="#-reproducible-builds-and-ci">CI</a> ·
  <a href="#-documentation">Documentation</a>
</p>

OmniDoc turns Pandoc Markdown or LaTeX into publication-ready documents without
inventing another markup language. It coordinates Pandoc, LaTeX, themes,
diagrams, citations, Lua filters, caching, validation, and release packaging
behind one focused CLI.

```text
Markdown / LaTeX + references + figures
                    │
                    ▼
      OmniDoc ── themes ── plugins ── cache/lock
                    │
        ┌───────────┼───────────┬──────────┬──────────┐
        ▼           ▼           ▼          ▼          ▼
       PDF         HTML        EPUB       DOCX       PPTX / LaTeX
```

## ✨ Why OmniDoc?

| | Capability | What it gives you |
|---|---|---|
| 📚 | **One source, many outputs** | Build PDF, HTML, EPUB, DOCX, PPTX, and LaTeX from one Markdown project. |
| 🌏 | **CJK-ready publishing** | Unicode-first PDF builds, Chinese language defaults, xeCJK, and Noto CJK font support. |
| 🎨 | **Cross-format themes** | Versioned themes can combine semantic tokens, CSS, LaTeX, templates, and Office reference documents. |
| 🧩 | **Safe Pandoc Lua plugins** | Installable filters and commands with explicit trust, exact version pins, and payload-digest verification. |
| ⚡ | **Incremental builds** | Content-aware cache, authoritative depfiles, native file watching, and automatic invalidation. |
| 🔒 | **Reproducible delivery** | Lockfiles, toolchain fingerprints, BLAKE3 digests, build reports, and transactional publishing. |
| 🧠 | **Actionable diagnostics** | Compact errors map Pandoc and LaTeX failures back to Markdown source lines with context and guidance. |
| 📐 | **Technical figures** | Draw.io, Graphviz, PlantUML, bitfields, KiCad, SVG conversion, circuits, and SPICE plots. |
| 🤖 | **Automation-first CLI** | Stable JSON, dry-run and diff previews, strict CI gates, shell completion, and nested project discovery. |

## 📦 Installation

### Official releases

Download the archive for Linux x86_64, macOS x86_64/Apple Silicon, or Windows
x86_64 from [GitHub Releases](https://github.com/wang-borong/omnidoc/releases),
place `omnidoc` on your `PATH`, then install the matching verified library:

```bash
omnidoc lib install
omnidoc doctor --strict
```

Official archives include a validated Tectonic PDF engine. OmniDoc still needs
[Pandoc](https://github.com/jgm/pandoc/releases) and
[pandoc-crossref](https://github.com/lierdakil/pandoc-crossref/releases).
Install Noto CJK fonts for Chinese, Japanese, or Korean PDFs. On Linux hosts
where the bundled Tectonic binary cannot run, OmniDoc falls back to Tectonic on
`PATH`, then XeLaTeX.

### Build from source

Rust 1.88 or newer is required:

```bash
git clone https://github.com/wang-borong/omnidoc.git
cd omnidoc
cargo build --locked --release
./target/release/omnidoc lib install
```

TeX Live is optional for Markdown projects, but remains useful for raw LaTeX
projects that require `latexmkrc`, Biber, specialized packages, or unsupported
shell-escape workflows.

## 🚀 Quick start

Create a Git-backed documentation project and build every default output:

```bash
omnidoc new my-book --type ctex-md --author "Docs Team"
cd my-book
omnidoc build --all --report --write-lock
omnidoc status
omnidoc open --to pdf
```

Prefer an interactive template selector? Use `omnidoc new my-book`. Already
have a repository? Run `omnidoc init`. Both commands support safe previews:

```bash
omnidoc new my-book --type ctex-md --dry-run --json
omnidoc init existing-docs --type ctex-md --diff
```

For one-off conversion, no project is required:

```bash
omnidoc convert pdf -l cn AI-usage.md
omnidoc convert html README.md -o README.html
```

## 📤 Output formats

| Output | Highlights |
|---|---|
| **PDF** | Tectonic or XeLaTeX/LuaLaTeX/PDFLaTeX, CJK, math, citations, custom LaTeX, embedded fonts. |
| **HTML** | Responsive CSS, MathML, semantic blocks, syntax highlighting, custom templates. |
| **EPUB 3** | Packaged CSS and assets, MathML, optional Readium compatibility validation. |
| **DOCX** | Theme-provided or project-provided reference documents. |
| **PPTX** | Presentation reference documents and format-aware figures. |
| **LaTeX** | Inspectable generated source for downstream TeX workflows. |

Markdown projects support every format above. Native LaTeX projects build PDF
through `latexmk` or a directly managed engine.

## 🛠️ Everyday workflow

| Goal | Command |
|---|---|
| Create or adopt a project | `omnidoc new PATH` · `omnidoc init PATH` |
| Build one or many formats | `omnidoc build --to html` · `omnidoc build --all` |
| Rebuild while editing | `omnidoc watch --all` |
| Inspect or open artifacts | `omnidoc status --json` · `omnidoc open --to pdf` |
| Format sources safely | `omnidoc fmt --check .` · `omnidoc fmt --diff main.md` |
| Validate and test | `omnidoc check lint` · `omnidoc check lock` · `omnidoc check ci` |
| Generate figures | `omnidoc figure diagram.drawio --format pdf` |
| Publish a release | `omnidoc publish --all --tag v1` |
| Preview cleanup/update | `omnidoc clean --dry-run` · `omnidoc update --diff` |
| Inspect configuration | `omnidoc config show --scope merged --json` |

Project-aware commands locate the nearest `.omnidoc.toml`, so they also work
from nested directories. Management commands expose machine-readable JSON;
mutating workflows use atomic writes and offer `--dry-run` or `--diff` where a
preview is meaningful.

## ⚙️ Project configuration

A compact `.omnidoc.toml` can describe a multi-format publication:

```toml
[project]
entry = "main.md"
target = "engineering-guide"

[build]
outputs = ["pdf", "html", "epub", "docx"]

[theme]
name = "engineering-book"
version = "=1.1.0"
compatibility = "readium"

[pandoc]
toc = true
css = "styles/project.css"
reference_doc = "styles/reference.docx"

[plugins]
# Install and trust the exact package before enabling it.
enabled = ["omnidoc/quality-gate@=1.0.0"]
```

CLI edits preserve TOML comments and layout:

```bash
omnidoc config set build.outputs '["pdf", "html", "epub"]'
omnidoc config set theme.name engineering-book --diff
omnidoc config unset pandoc.css --dry-run
```

## 🎨 Themes, plugins, and templates

### Themes

Themes are declarative and never execute Lua. A theme can target selected
outputs and provide inherited design tokens, HTML/EPUB CSS, LaTeX resources,
Pandoc templates, DOCX/PPTX reference documents, metadata defaults, fonts, and
compatibility requirements.

```bash
omnidoc theme list
omnidoc theme install ./corporate-theme --project ./docs
omnidoc theme inspect acme/corporate@^2 --project ./docs
omnidoc theme apply acme/corporate@=2.1.0 ./docs
omnidoc theme validate --check-fonts --check-latex
```

Built-in profiles include `engineering-book`, `corporate-docs`,
`classic-book`, `clean-document`, and `modern-slides`. See the
[theme package guide](bundles/libs/THEMES.md).

### Plugins

OmniDoc deliberately has no host Lua VM and no generic lifecycle hooks.
Automatic extensions are Pandoc `--lua-filter` packages; explicit commands run
through `pandoc lua SCRIPT`.

```bash
omnidoc plugin install-example quality-gate --project ./docs
omnidoc plugin validate omnidoc/quality-gate@=1.0.0 --project ./docs --check-lua
omnidoc plugin trust omnidoc/quality-gate@=1.0.0 --project ./docs
omnidoc plugin enable omnidoc/quality-gate@=1.0.0 ./docs
```

Installation alone never executes code. Automatic filters require both local
trust of the exact package digest and an exact project version pin. Replacing
the payload invalidates trust. Learn more in the
[plugin package guide](bundles/libs/PLUGINS.md).

### Templates

Built-in and external Tera templates share the same interactive and scriptable
creation flow. External templates are hot-loaded from `OMNIDOC_TEMPLATE_DIR`
or the configured `template_dir`.

```bash
omnidoc template list
omnidoc template validate --json
omnidoc new handbook --type simple-md
```

## 📐 Rich authoring and figures

OmniDoc's managed Pandoc pipeline supports citations and cross-references,
MathML/LaTeX math, recursive file and code inclusion, admonitions, emoji,
alignment and font controls, semantic containers, syntax highlighting, and
format-aware figures.

Diagram sources are first-class dependencies: when an included file, image,
filter input, bibliography, or generated figure changes, the cache and lockfile
change with it.

```bash
omnidoc figure bitfield registers.json --format svg --beautify
omnidoc figure dot architecture.dot --format pdf
omnidoc figure plantuml sequence.puml --format png
omnidoc figure kicad board.kicad_sch --format svg --exclude-drawing-sheet
omnidoc figure convert diagram.svg --format pdf
```

Markdown fenced blocks can also render bitfields, circuits, and SPICE plots to
PDF for print and SVG for web/office outputs. See the complete
[semantic block syntax](bundles/libs/BLOCKS.md).

Optional renderers include Draw.io, Graphviz, PlantUML, KiCad CLI, Inkscape,
ImageMagick, Schemdraw, and ngspice. Install only the tools your documents use.

## 🔒 Reproducible builds and CI

- **Cache:** BLAKE3 fingerprints cover sources, resolved resources,
  configuration, extensions, and toolchain versions.
- **Dependencies:** Pandoc filters and TeX recorder files produce authoritative
  depfiles for recursive includes and files actually consumed by LaTeX.
- **Lock:** `omnidoc.lock` records the resolved multi-output dependency graph.
- **Report:** `build/omnidoc-report.json` explains cache decisions, timings,
  digests, tools, artifacts, and EPUB validation.
- **Watch:** the native watcher tracks project, library, configuration, theme,
  plugin, and external dependency changes without output feedback loops.
- **Publish:** releases are assembled transactionally and verified by exact
  file set, size, digest, and library contract.

```bash
# Local quality gate
omnidoc fmt --check .
omnidoc check ci

# Reproducible release
omnidoc publish --all --tag v1.0.0
omnidoc publish --verify --tag v1.0.0 --json
```

For repository development, the full gates are:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
scripts/check-golden-book.sh
scripts/check-golden-pdf.sh
```

## 📁 Project layout

```text
.omnidoc.toml       Project contract
main.md             Markdown entry (or main.tex)
biblio/             Bibliographies
drawio/             Diagram sources
figure/             Versioned/static assets
figures/            Generated figures
md/ and tex/        Included source files
build/              Generated artifacts and report
omnidoc.lock        Reproducible dependency snapshot
```

`new`, `init`, and `update` integrate with Git and refuse to mix unrelated
dirty work into automatic commits. Source moves and managed-file updates can be
reviewed before anything is written.

## 📖 Documentation

- [`omnidoc --help`](src/cli/commands.rs) — complete command discovery
- [Themes](bundles/libs/THEMES.md) — package schema, resources, inheritance, and tokens
- [Plugins](bundles/libs/PLUGINS.md) — Pandoc Lua package and trust model
- [Semantic blocks](bundles/libs/BLOCKS.md) — portable authoring extensions
- [Tectonic engine policy](docs/decisions/0001-tectonic-engine-policy.md) — capabilities and compatibility boundary
- [Release checklist](release/CHECKLIST.md) — packaging and acceptance gates
- [Changelog](CHANGELOG.md) — release history

## 📄 License

OmniDoc is available under the [MIT License](LICENSE).
