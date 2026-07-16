# omnidoc

This is a wrapper, based on Pandoc and LaTeX, for a documentation writing system that helps manage document repositories.
With omnidoc, you can write in Pandoc markdown or LaTeX and convert these files to PDF, HTML, EPUB, DOCX, or LaTeX outputs easily.
To use this tool, you need to learn how to write in [Pandoc markdown](https://pandoc.org/MANUAL.html#pandocs-markdown) or [LaTeX](https://www.overleaf.com/learn/latex/Learn_LaTeX_in_30_minutes).

## Dependencies

- **Pandoc**  

  Download Pandoc and pandoc-crossref from GitHub releases: 

  - [Pandoc](https://github.com/jgm/pandoc/releases)  
  - [pandoc-crossref](https://github.com/lierdakil/pandoc-crossref/releases)

- **LaTeX**  

  You can install LaTeX following [this manual](https://www.tug.org/texlive/quickinstall.html) or through your Linux distribution's package manager.

  For example, on Arch Linux, you should install these packages:

  ```
  texlive-basic
  texlive-bibtexextra
  texlive-bin
  texlive-fontsrecommended
  texlive-langchinese
  texlive-langcjk
  texlive-latex
  texlive-latexextra
  texlive-latexrecommended
  texlive-mathscience
  texlive-pictures
  texlive-plaingeneric
  texlive-xetex
  ```

- **Tectonic (optional)**  

  Tectonic can be used as a lighter PDF engine. It downloads missing TeX packages on demand and can be selected per build:

  ```bash
  omnidoc build --pdf-engine tectonic
  ```

  For raw LaTeX projects, the default `latexmk` backend is still recommended when you depend on custom `latexmkrc` rules, external bibliography tools, or shell-escape workflows.

- **Draw.io**  

  Download Draw.io from its [GitHub releases](https://github.com/jgraph/drawio-desktop/releases).

- **Graphviz**  

  Install it through your Linux distribution's package manager.

- **Inkscape**  

  Install it through your Linux distribution's package manager.

- **ImageMagick**  

  Install it through your Linux distribution's package manager.

- **PlantUML**

  Install it through your Linux distribution's package manager.

## Usage

### Quick Start

1. **Create a new documentation repository**

   ```bash
   omnidoc new <PATH> --title "Document Title" [--author "Author Name"]
   ```

   Example:
   ```bash
   omnidoc new hello --title "My Document" --author "John Doe"
   ```

   You'll be prompted to choose a template (built-in + external) with an interactive selector. Use arrow keys to navigate, Enter to select. You can also type an external template key directly (e.g., `simple-md`).

   Example (inquire-based selection):

   ```
   $ omnidoc new hello --title "hello"

   ? Select document type:
   > ctex-md — ctex class based markdown document writing system
     ebook-md — elegantbook class based markdown document writing system
     enote-md — elegantnote class based markdown document writing system
     ctexart-tex — raw ctexart document type
     ctexrep-tex — raw ctexrep document type
     ctexbook-tex — raw ctexbook document type
     ebook-tex — elegantbook class based latex document writing system
     enote-tex — elegantnote class based latex document writing system
     ctart-tex — ctart class based latex document writing system
     ctrep-tex — ctrep class based latex document writing system

   [Use arrow keys to navigate, Enter to confirm, Esc/Ctrl+C to cancel]
   ```

   The suffixes `-tex` and `-md` indicate the text format for built-in types; external templates are shown under "External templates".

   After selecting a template, the tool creates the repository with the following structure:

   ```
   biblio/     # Bibliography files (.bib)
   dac/        # D2 diagram source files
   drawio/     # Draw.io diagram source files
   figure/     # Generated figure output directory
   figures/    # Third-party figure files
   md/         # Additional markdown files (for markdown projects)
   tex/        # Additional LaTeX files (for LaTeX projects, if configured)
   main.md     # Main entry file (or main.tex for LaTeX projects)
   ```

   The project is automatically initialized as a git repository.

2. **Initialize an existing repository**

   If you have an existing directory with markdown or LaTeX files, you can initialize it as an omnidoc project:

   ```bash
   omnidoc init [PATH] --title "Document Title" [--author "Author Name"]
   ```

   If `PATH` is not specified, the current directory is used. The tool will:
   - Prompt you to select a document type
   - Move existing `.md` and `.tex` files to appropriate directories
   - Create the directory structure
   - Initialize git repository if not already present

3. **Build the repository**

   Build your content. Markdown projects can output `pdf`, `html`, `epub`, `docx`, or `latex`; LaTeX projects output PDF.

   ```bash
   omnidoc build [PATH] [--to <FORMAT>] [--output <FORMAT>]... [--all] [OPTIONS]
   ```

   - If `PATH` is not specified, the current directory is used
   - Use `--to html`, `--to epub`, `--to docx`, or `--to latex` for Markdown project builds
   - Use repeated `--output <FORMAT>` to build a specific set of outputs
   - Use `--all` to build `[build].outputs` or the default set: `pdf`, `html`, `docx`, `epub`
   - Use `--pdf-engine tectonic` to compile PDFs with Tectonic instead of XeLaTeX
   - Use `--latex-backend engine --max-latex-passes 5` for direct XeLaTeX/LuaLaTeX/PDFLaTeX builds that stop when `.aux/.toc`-style files stop changing
   - Keep the default `--latex-backend latexmk` when you need bibliography/glossary automation or custom `.latexmkrc` rules
   - Use `--force` to ignore the `.omnidoc-cache` input/config hash and rebuild
   - Use `--report` to write `build/omnidoc-report.json`
   - Use `--write-lock` to update `omnidoc.lock` after a successful build
   - Use `--strict` to fail on lint/config warnings before building
   - Use `--verbose` to show detailed build messages
   - The build directory is `build/` (configurable via config), and the output file is named after the repository directory

   Build reports include the cache decision reason and component-level
   `cache_details`, elapsed milliseconds, input and artifact BLAKE3 digests,
   resolved resource digests, and detected Pandoc/pandoc-crossref/LaTeX
   toolchain versions. Cache details identify added, removed, or changed
   dependencies, resources, configuration fields, and toolchain components.
   Cache schema v6 stores these component fingerprints locally; older cache
   records are rebuilt automatically and reported as `cache_schema_changed`.

   The Markdown and code include filters emit authoritative depfiles under
   `.omnidoc-cache/`. After the first successful build, recursive files that
   were actually transcluded are used directly by cache, report, and lock
   generation. The initial build retains a conservative source scan so no
   separate dependency-generation step is required.

   Examples:

   ```bash
   omnidoc build
   omnidoc build --to html
   omnidoc build --output pdf --output docx
   omnidoc build --all --report --write-lock
   omnidoc build --to docx
   omnidoc build --pdf-engine tectonic
   omnidoc build --latex-backend engine --pdf-engine xelatex
   ```

   Build failures include a compact Pandoc/LaTeX diagnostic summary so the first relevant error is visible without reading the full `.log` file. For Markdown projects, OmniDoc also maps Pandoc/LaTeX errors back to structured source diagnostics:

   ```text
   Markdown source diagnostic: main.md:42:7: undefined_control_sequence
     |
   41 | Intro text before the equation.
   42 | $ \badmacro $
      |       ^
   43 | Follow-up paragraph.
     note: ! Undefined control sequence.
     help: Check raw LaTeX commands, math macros, and required packages near this Markdown location.
   ```

   The mapper understands direct `file:line:column` diagnostics, Pandoc `line/column`
   parse errors, common LaTeX log fragments such as `l.<line>`, missing resources,
   citation keys, missing packages, and Unicode-character failures. It searches the
   entry file first, then project Markdown files while skipping build/cache outputs.

   You can also persist build choices in `.omnidoc.toml`:

   ```toml
   [project]
   entry = "main.md"
   from = "markdown"
   to = "html"
   target = "manual"

   [tools]
   latex_engine = "tectonic"
   # tectonic = "/custom/path/to/tectonic"

   [build]
   outputs = ["pdf", "html", "docx"]
   latex_backend = "engine"
   max_latex_passes = 5

   [theme]
   name = "engineering-book"
   version = "1"
   compatibility = "readium"

   [pandoc]
   css = "styles/manual.css"
   html_template = "templates/page.html"
   latex_template = "templates/report.tex"
   epub_template = "templates/book.html"
   reference_doc = "templates/reference.docx"
   epub_css = "styles/epub.css"

   [pandoc.format_options]
   # Writer-specific options are appended after common `pandoc.options`.
   # This is useful when the same setting has different semantics by writer.
   epub = ["--toc-depth=3"]
   pdf = ["--toc-depth=1"]
   ```

   `template` is still accepted as a generic fallback for template-capable outputs. DOCX uses `reference_doc` instead of Pandoc `--template`.

   A selected theme supplies default HTML/EPUB CSS and required Lua filters.
   Explicit `[pandoc]` resource settings retain higher priority. The selected
   theme manifest and every resource actually consumed by the output are part
   of the lock/cache input digest; changing the bundle invalidates the cache.

   `compatibility = "readium"` activates the versioned Readium EPUB profile.
   Every EPUB build then validates the ZIP/mimetype contract, normalized and
   unique entry paths, EPUB 3 package/navigation documents, packaged CSS and
   local resources, MathML namespaces, and hidden TeX annotations. Validation
   results and the Readium/Thorium, Calibre, and Apple Books target matrix are
   written to `omnidoc-report.json`; an invalid artifact is never cached. This
   deterministic profile gate complements EPUBCheck, which remains mandatory
   in the Golden Book GitHub Actions job.

   HTML and EPUB builds also load OmniDoc's portable base stylesheet before
   the selected theme. It provides semantic layout primitives such as centered
   standalone formulas while leaving inline math unchanged.

4. **Watch and rebuild while editing**

   ```bash
   omnidoc watch [PATH] [--to <FORMAT>] [--output <FORMAT>]... [--all] [--debounce-ms 250]
   ```

   `watch` uses the native `notify` backend, rebuilds once immediately, then rebuilds on source changes such as `.md`, `.tex`, `.bib`, `.drawio`, `.dot`, `.json`, and common image assets. Build failures are printed and the watcher keeps running. There is no polling fallback.

5. **Publish generated artifacts**

   ```bash
   omnidoc publish [PATH] [--to <FORMAT>] [--all] [--tag <TAG>] [--dist-dir dist]
   omnidoc publish [PATH] --verify --tag <TAG> [--dist-dir dist] [--json]
   ```

   `publish` builds by default, writes `omnidoc.lock` and
   `build/omnidoc-report.json`, then copies generated artifacts into
   `dist/<tag-or-target>/`. The v2 `omnidoc-publish.json` manifest uses portable
   paths and records the byte size and BLAKE3 digest of every copied artifact.
   It also embeds and publishes `omnidoc-libs.toml`, binding the document
   release to the compatible library archive and checksum. Use `--no-build` to
   publish existing build outputs. Publication is transactional: files are
   assembled and hashed in a sibling staging directory, then replace the final
   tag directory only after the complete manifest is written. Failed publishes
   preserve the previous release, while successful republishes remove stale
   artifacts. `--verify` rechecks the manifest schema, portable paths, exact
   file set, byte sizes, BLAKE3 digests, and embedded libs release contract.

6. **Open the built PDF document**

   ```bash
   omnidoc open [PATH]
   ```

   Opens the built PDF document using the system's default PDF viewer.

7. **Clean the repository**

   Remove build artifacts:

   ```bash
   omnidoc clean [PATH] [--distclean]
   ```

   - `clean`: Removes the build directory
   - `clean --distclean`: Removes build directory and all generated files

### Project Management Commands

7. **Update a document repository**

   Update an existing omnidoc project structure:

   ```bash
   omnidoc update [PATH]
   ```

   This command updates the project structure, template files, and configuration to match the current omnidoc version.

8. **List all supported document types**

   Preview available built-in types and external templates:

   ```bash
   omnidoc list
   ```

   This displays all built-in document types and external templates that are available for selection.

### Configuration Commands

9. **Generate default configuration**

   Create or update the global configuration file:

   ```bash
   omnidoc config --authors "Author Name" [OPTIONS]
   ```

   Options:
   - `--authors <AUTHORS>`: Configure the author name (required)
   - `--lib <LIB>`: Configure the OmniDoc library path
   - `--outdir <OUTDIR>`: Configure the output directory for building (default: `build`)
   - `--texmfhome <TEXMFHOME>`: Configure the TEXMFHOME environment variable
   - `--bibinputs <BIBINPUTS>`: Configure the BIBINPUTS environment variable
   - `--texinputs <TEXINPUTS>`: Configure the TEXINPUTS environment variable
   - `--force`: Force generation (overwrite existing config)

   Example:
   ```bash
   omnidoc config --authors "John Doe" --outdir "output" --lib "$HOME/.local/share/omnidoc"
   ```

10. **Maintain the OmniDoc library**

   Install, update, inspect, or verify the OmniDoc library files (`libs` is a
   visible alias of `lib`):

   ```bash
   omnidoc lib --install       # Install to the configured library path
   omnidoc lib --update        # Pull main, then verify the installed payload
   omnidoc lib --install --revision v1.0.0
   omnidoc lib --update --revision 428c8e6
   omnidoc lib --install --release # Download the archive bound to this OmniDoc release
   omnidoc lib --update --release  # Verify checksum and replace transactionally
   omnidoc libs --status       # Show version, revision and compatibility
   omnidoc libs --status --revision v1.0.0
   omnidoc libs --verify       # Verify required files and every SHA-256 entry
   omnidoc libs --verify --json
   ```

   Install and update fail if the downloaded manifest, compatibility contract,
   required resources, payload checksums, or requested revision do not verify.
   Set `revision = "v1.0.0"` under `[lib]` in the global configuration to pin
   all subsequent install, update, status, and verify operations. Updates also
   refuse to overwrite a dirty library checkout. Install and update are
   transactional: OmniDoc clones into a sibling staging directory, validates
   the complete bundle, and only then replaces the active library.
   With `--release`, OmniDoc instead reads its embedded `omnidoc-libs.toml`
   contract, downloads the matching `.tar.gz` and external SHA-256 file,
   rejects unsafe archive entries, verifies the internal manifest/checksums,
   and promotes the extracted bundle using the same transactional replacement.
   Later `omnidoc lib --update` calls remember an archive installation and use
   the matching release source automatically unless a Git revision is requested.

11. **Inspect versioned theme bundles**

   Theme bundles are declared by `themes/<name>.toml` inside omnidoc-libs and
   can bind matching HTML CSS, EPUB CSS, LaTeX packages, Lua filters, font
   requirements, metadata defaults, and an OmniDoc compatibility range:

   ```bash
   omnidoc theme list
   omnidoc theme inspect engineering-book
   omnidoc theme validate engineering-book
   omnidoc theme validate engineering-book --check-fonts
   omnidoc theme validate engineering-book --check-fonts --check-latex
   omnidoc theme validate --json       # validate every installed theme
   ```

   ```toml
   manifest_version = 1
   name = "engineering-book"
   version = "1.0.0"
   compatible_omnidoc = ">=1.3.0,<2.0.0"
   compatibility = "readium"

   [resources]
   html_css = ["pandoc/css/engineering-book.css"]
   epub_css = ["pandoc/css/engineering-book.css"]
   latex_packages = ["texmf/tex/common/omni-engineering-book.sty"]
   latex_headers = ["pandoc/headers/engineering-book.tex"]
   latex_template = "pandoc/data/templates/pantext.latex"
   lua_filters = ["pandoc/data/filters/admonition.lua"]

   [requirements]
   fonts = ["Noto Serif CJK SC"]
   system_latex_packages = ["fontspec", "xeCJK", "tcolorbox"]

   [metadata.defaults]
   lang = "zh-CN"
   ```

   Validation rejects incompatible versions, missing or duplicate resources,
   unsafe paths, and symbolic links in the bundle contract. `--check-fonts`
   additionally resolves every declared font family with fontconfig and rejects
   silent fallback matches; the Golden PDF gate requires this environment
   check. `--check-latex` resolves every declared system package with
   `kpsewhich`. PDF lock/cache entries include the TeX distribution identity
   plus each resolved `.sty` version, file name, and BLAKE3 digest.

   Theme metadata defaults are emitted as Pandoc `-M key=value` arguments for
   projects without an authoritative `build.metadata_file`. Explicit
   `pandoc.lang`, project author/title defaults, and user-supplied Pandoc
   options take precedence. Metadata keys are validated as portable scalar
   identifiers and values must remain single-line strings.

   `html_template`, `epub_template`, and `latex_template` bind a template to a
   specific writer. Explicit project format templates and the generic
   `pandoc.template` override the theme; PDF/LaTeX falls back to
   `pantext.latex` only when neither project nor theme selects one.

### Project Quality and CI Commands

Run environment diagnostics:

```bash
omnidoc doctor [PATH]
omnidoc doctor --json
```

Validate project configuration:

```bash
omnidoc config-validate [PATH]
```

Lint source references and configured resources:

```bash
omnidoc lint [PATH] [--strict]
```

Inspect the tracked dependency graph used by cache, reports, and lock files:

```bash
omnidoc deps [PATH]
omnidoc deps --json
```

The graph merges project references with the latest include-filter depfiles;
external included files are recorded as content-digested resources rather
than machine-specific project paths.

Every active Lua filter also receives
`omnidoc-depfile-<normalized-filter-stem>` metadata pointing at its own `.d`
file under `.omnidoc-cache`. A third-party filter can write the
`# omnidoc-depfile-v1` header followed by each file it actually read. Only
depfiles belonging to filters active for the current output are consumed, so
stale or unrelated filter data cannot pollute the target dependency graph.

Create or refresh the lock file:

```bash
omnidoc lock [PATH]
omnidoc lock --update
omnidoc lock --check
```

`lock --check` exits with an error when `omnidoc.lock` is missing or stale.
Lock schema v4 uses BLAKE3 content digests and stores dependencies and resolved
resources separately for every configured output target. It also records the
selected omnidoc-libs revision/content digest and detected Pandoc,
pandoc-crossref, LaTeX engine, and PDF theme font identities. Font identities
include the resolved family, style, font version, file name, and content digest.
Toolchain identities now participate in cache keys, so replacing Pandoc,
XeLaTeX, or a font invalidates existing outputs. When the library bundle
provides a manifest, its declared version plus manifest/checksum digests are
locked as well. Older lock files must be regenerated with
`omnidoc lock --update`.

For XeLaTeX, pdfLaTeX, and LuaLaTeX PDF builds, OmniDoc also enables the TeX
recorder and converts the engine's `.fls` file into
`.omnidoc-cache/latex-inputs.d`. Project `\input` files and indirect system
resources actually loaded by the engine are content-hashed on later cache and
lock checks. The first successful build adopts the depfile before writing its
cache entry, so it does not require a second build to stabilize.

Run CI-mode validation and builds:

```bash
omnidoc ci [PATH] [--output pdf] [--output html]
```

`ci` runs strict validation, builds all configured/default outputs, writes `build/omnidoc-report.json`, and updates `omnidoc.lock`.

Run the real Pandoc Golden Book gate locally before release-oriented changes:

```bash
OMNIDOC_LIBS=../omnidoc-libs scripts/check-golden-book.sh
OMNIDOC_LIBS=../omnidoc-libs scripts/check-golden-pdf.sh
```

The PDF gate also renders every page at a fixed DPI and checks the committed
page-aware visual contract (page count, canvas, ink bounds/coverage, and a
perceptual hash). Nightly and release CI retain the PDF, rendered PGM pages,
font inventory, extracted text, and JSON comparison report as a diagnostic
artifact. After an intentional layout change, review the render and refresh
the contract explicitly with:

```bash
OMNIDOC_LIBS=../omnidoc-libs \
OMNIDOC_PDF_VISUAL_MODE=capture \
scripts/check-golden-pdf.sh
```

The gate builds HTML and EPUB from a recursive-include fixture, checks MathML,
display-math layout, repeated heading IDs, packaged CSS/images, lock/report
digests, and shared-resource cache invalidation. It also runs EPUBCheck when the
`epubcheck` executable is installed.

The PDF gate additionally exercises XeLaTeX, the engineering-book LaTeX
package, CJK text, admonitions, deterministic SVG-to-PDF sibling assets, page
generation, embedded/subset fonts, lock contents, and cache invalidation.

GitHub Actions runs the same gate with pinned Pandoc and pandoc-crossref
versions and requires EPUBCheck, so pull requests exercise the real HTML/EPUB
toolchain rather than only the Rust command-construction layer.
The heavier PDF gate runs weekly, for version tags, and when manually
dispatched.

Official OmniDoc archives and Debian packages include
`omnidoc-libs.toml`, a machine-readable release contract declaring the matching
library version/tag, release archive URL, and external SHA-256 URL. CI compares
that contract with OmniDoc's Cargo version and the checked-out
`omnidoc-libs/manifest.toml` before packaging. Verify it locally with:

```bash
python3 scripts/check-library-contract.py ../omnidoc-libs
```

List discovered local plugins and external template manifests:

```bash
omnidoc plugin [PATH]
omnidoc plugin --json
omnidoc plugin --validate
```

`plugin --validate` parses discovered `manifest.toml` files and checks template plugin fields such as `language` and `template_file`.
`plugin --json` also reports declared hooks, and validation checks local hook command paths when the command contains a path separator.

### Document Formatting Commands

11. **Format documents**

    Format markdown or LaTeX documents recursively:

    ```bash
    omnidoc fmt [PATHS...] [OPTIONS]
    ```

    Options:

    - `--backup`: Create backup files before formatting
    - `--semantic`: Enable semantic formatting
    - `--symbol`: Enable symbol formatting (Chinese punctuation)

    The default formatter is conservative and block/token-aware. It leaves
    front matter, fenced and indented code, display math, nested/custom raw
    HTML containers, pipe/grid tables, definition lists, block-level raw TeX,
    reference definitions, nested LaTeX environments, inline code/math,
    escapes, raw inline HTML, balanced links/images, reference and citation
    labels, and Pandoc attribute blocks byte-stable. Nested parentheses in
    destinations are parsed structurally rather than with URL regexes. `.tex`
    files use a separate mode that protects command and environment lines.
    `--semantic` and `--symbol` remain explicit opt-ins, and repeated formatting
    is required to be idempotent.

    Examples:

    ```bash
    omnidoc fmt main.md                    # Format a single file
    omnidoc fmt md/                        # Format all files in md directory
    omnidoc fmt --backup --semantic .      # Format all files in current directory with backup
    ```

### Figure Generation Commands

12. **Generate figures from source files**

    Generate figures from various diagram source formats:

    ```bash
    omnidoc figure [SOURCES...] [OPTIONS] [COMMAND]
    ```

    General options:
    - `--format <FORMAT>`: Output format (pdf, png, svg, etc.), default: pdf
    - `--force`: Force regenerate even if output exists
    - `--output <OUTPUT>`: Output directory

    If no subcommand is specified, the tool will auto-detect the figure type based on file extension.

    **Subcommands:**

    - **Generate bitfield diagrams from JSON**

      ```bash
      omnidoc figure bitfield <SOURCES>... [OPTIONS]
      ```

      Options:
      - `--vspace <VSPACE>`: Vertical space
      - `--hspace <HSPACE>`: Horizontal space
      - `--lanes <LANES>`: Rectangle lanes
      - `--bits <BITS>`: Overall bitwidth
      - `--fontfamily <FONTFAMILY>`: Font family (default: sans-serif)
      - `--fontsize <FONTSIZE>`: Font size (default: 14)
      - `--strokewidth <STROKEWIDTH>`: Stroke width (default: 1.0)
      - `--beautify`: Beautify output
      - `--json5`: Use JSON5 parser
      - `--compact`: Compact mode
      - `--hflip`: Horizontal flip
      - `--vflip`: Vertical flip
      - `--trim <TRIM>`: Trim long bitfield names (character width)
      - `--uneven`: Uneven lanes
      - `--legend <LEGEND>`: Legend item (format: NAME:TYPE, can be used multiple times)

      Bitfield JSON sources can embed the same rendering options so you don't have to repeat CLI flags. Define an object that includes the option names alongside an `entries` array (aliases: `bitfields`, `fields`, `items`, `data`):

      ```json
      {
        "vspace": 130,
        "beautify": true,
        "legend": {
          "LEN": "Frame length",
          "ST": "Trigger status"
        },
        "entries": [
          { "bits": 8, "name": "LEN", "attr": "0" },
          { "bits": 1, "name": "ST", "attr": ["0: no trigger", "1: triggered"] }
        ]
      }
      ```

      CLI flags always override the values stored in the JSON document.

    - **Generate diagrams from Draw.io files**

      ```bash
      omnidoc figure drawio <SOURCES>... [OPTIONS]
      ```

      Options:
      - `--drawio <DRAWIO>`: Draw.io executable path
      - `--format <FORMAT>`: Output format (default: pdf)

    - **Generate diagrams from Graphviz dot files**

      ```bash
      omnidoc figure dot <SOURCES>... [OPTIONS]
      ```

      Options:
      - `--gradot <GRADOT>`: Graphviz dot executable path
      - `--format <FORMAT>`: Output format (default: pdf)

    - **Generate diagrams from PlantUML files**

      ```bash
      omnidoc figure plantuml <SOURCES>... [OPTIONS]
      ```

      Options:
      - `--plantuml <PLANTUML>`: PlantUML executable path or jar file path
      - `--format <FORMAT>`: Output format (default: png)

    - **Convert images**

      Convert images between different formats (SVG, PDF, PNG, etc.):

      ```bash
      omnidoc figure convert <SOURCES>... [OPTIONS]
      ```

      Options:
      - `--inkscape <INKSCAPE>`: Inkscape executable path
      - `--imagemagick <IMAGEMAGICK>`: ImageMagick executable path
      - `--format <FORMAT>`: Output format (default: pdf)

    Examples:
    ```bash
    # Auto-detect and generate from drawio file
    omnidoc figure diagram.drawio --format pdf

    # Generate bitfield diagram from JSON
    omnidoc figure bitfield bitfield.json --format svg --beautify

    # Convert SVG to PDF
    omnidoc figure convert figure.svg --format pdf

    # Generate all figures in a directory
    omnidoc figure drawio/ --format pdf --output figure/
    ```

### Document Conversion Commands

13. **Convert markdown to PDF**

    Convert markdown files directly to PDF without creating a full project:

    ```bash
    omnidoc md2pdf <INPUTS>... [OPTIONS]
    ```

    Options:
    - `--lang <LANG>`: Language (cn or en)
    - `--output <OUTPUT>`: Output file path

    Examples:
    ```bash
    omnidoc md2pdf document.md --lang cn --output document.pdf
    omnidoc md2pdf file1.md file2.md --output combined.pdf
    ```

14. **Convert markdown to HTML**

    Convert markdown files to HTML:

    ```bash
    omnidoc md2html <INPUTS>... [OPTIONS]
    ```

    Options:
    - `--output <OUTPUT>`: Output file path (for single input) or directory (for multiple inputs)
    - `--css <CSS>`: CSS file path for styling

    Examples:
    ```bash
    omnidoc md2html document.md --output document.html
    omnidoc md2html file1.md file2.md --output html/ --css style.css
    ```

### Template Management Commands

15. **Template toolkit**

    Validate external template manifests and files:

    ```bash
    omnidoc template --validate
    ```

    This command validates all external templates (hot-loaded, no restart needed). It checks:
    - Manifest parsing
    - Template file existence
    - Minimal Tera render with `title/author/date`

### Utility Commands

16. **Generate shell completion**

    Generate shell completion scripts for bash, zsh, fish, elvish, or PowerShell:

    ```bash
    omnidoc complete --generate <SHELL>
    ```

    Supported shells: `bash`, `zsh`, `fish`, `elvish`, `powershell`

    Example:
    ```bash
    # For zsh
    omnidoc complete --generate zsh > ~/.zsh_completions/_omnidoc

    # For bash
    omnidoc complete --generate bash > ~/.bash_completion.d/omnidoc
    ```

## Dynamic Templates (External)

omnidoc supports dynamic, user-defined templates without changing code. You can place template manifests and files under a template directory, and omnidoc will pick them up automatically (hot-loaded) when listing or validating.

### Configure the template directory

Priority order:
1) Environment variable: `OMNIDOC_TEMPLATE_DIR`
2) Config file field: `template_dir` in `~/.config/omnidoc.toml`

Example config (`~/.config/omnidoc.toml`):

```
[lib]
path = "/home/wbr/.local/share/omnidoc"

[env]
outdir = "build"
texmfhome = "$ENV{HOME}/.local/share/omnidoc/texmf//:"
texinputs = "./tex//:"
bibinputs = "./biblio//:"

template_dir = "/home/wbr/.local/share/omnidoc/templates"
```

You can also set an environment variable (takes precedence):

```bash
export OMNIDOC_TEMPLATE_DIR="/home/wbr/.local/share/omnidoc/templates"
```

### Directory layout and manifest

Two supported layouts (you can mix them):
- A) Centralized manifests under `manifests/`:
  - `{root}/manifests/{key}.toml`
  - The `template_file` path is relative to the manifest's directory
- B) One directory per template:
  - `{root}/{key}/manifest.toml`
  - The `template_file` is typically next to the manifest

Example:

```
/home/wbr/.local/share/omnidoc/templates/
  simple-md/
    manifest.toml
    template.md
  my-tex/
    manifest.toml
    template.tex
```

Manifest schema (`manifest.toml`):

```
key = "simple-md"                 # unique key used when selecting template
name = "Simple Markdown"          # optional, display name
description = "A minimal markdown doc template"  # optional
language = "markdown"             # "markdown" | "latex"
template_file = "template.md"     # relative to manifest directory
file_name = "main.md"             # optional; defaults: markdown->main.md, latex->main.tex

[hooks]
# Commands are executed without a shell. Use an array when arguments are needed.
asset_provider = ["scripts/assets.sh"]
pre_build = ["scripts/pre-build.sh"]
post_build = ["scripts/post-build.sh"]
lint_rule = ["scripts/lint.sh"]
```

Hook environment variables:
- `OMNIDOC_PROJECT_DIR`
- `OMNIDOC_PLUGIN_DIR`
- `OMNIDOC_PLUGIN_KEY`
- `OMNIDOC_HOOK`
- `OMNIDOC_OUTPUT`
- `OMNIDOC_TARGET`

`lint_rule` commands can print diagnostics in this format:

```text
warning:main.md:12:5:message from plugin
error:chapter.md:3:1:another message
```

### Template files

Templates are rendered with Tera. Available variables:
- `{{ title }}`
- `{{ author }}`
- `{{ date }}` (YYYY/MM/DD)

Example `template.md`:

```
---
title: {{ title }}
author:
  - {{ author }}
date:
  - {{ date }}

indent: true
toc: true
...

# {{ title }}

Welcome, {{ author }}!
```

Example `template.tex`:

```
\documentclass{article}
\title{ {{ title }} }
\author{ {{ author }} }
\date{ {{ date }} }
\begin{document}
\maketitle
\tableofcontents
\section{Intro}
Hello, {{ author }}.
\end{document}
```

### List and validate

- List built-in types and external templates:

```bash
omnidoc list
```

- Validate external templates (hot-loaded, no restart):

```bash
omnidoc template --validate
```

The validator checks manifest parsing, template file existence, and a minimal Tera render with `title/author/date`.

### Initialize with external templates

When prompted to choose a document type, you can type the external template `key` (e.g., `simple-md`, `my-tex`), or pick from the list if displayed.
