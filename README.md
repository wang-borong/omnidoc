# omnidoc

This is a wrapper, based on Pandoc and LaTeX, for a documentation writing system that helps manage document repositories.
With omnidoc, you can write in Pandoc markdown or LaTeX and convert these files to PDF, HTML, EPUB, DOCX, or LaTeX outputs easily.
To use this tool, you need to learn how to write in [Pandoc markdown](https://pandoc.org/MANUAL.html#pandocs-markdown) or [LaTeX](https://www.overleaf.com/learn/latex/Learn_LaTeX_in_30_minutes).

## Dependencies

- **Pandoc**  

  Download Pandoc and pandoc-crossref from GitHub releases: 

  - [Pandoc](https://github.com/jgm/pandoc/releases)  
  - [pandoc-crossref](https://github.com/lierdakil/pandoc-crossref/releases)

- **PDF engine and CJK fonts**

  Official OmniDoc binary archives include Tectonic 0.16.9. Markdown PDF
  builds automatically prefer this bundled engine, so users do not need to
  install TeX Live merely to build an OmniDoc-managed document. Tectonic
  downloads TeX packages into its cache on first use. Linux archives use
  Tectonic's official GNU release binary; the pinned 0.16.9 x86_64 asset
  requires glibc 2.39 or newer. If the bundled executable cannot run on the
  host, OmniDoc skips it and tries Tectonic from `PATH`, then XeLaTeX.
  Linux package metadata also declares the engine's OpenSSL 3, Graphite2,
  Brotli, Zstandard, libstdc++, and CA-certificate runtime requirements.

  CJK document fonts are system resources and are not embedded in the OmniDoc
  package. Install Noto CJK fonts (for example `fonts-noto-cjk` on Debian or
  `noto-fonts-cjk` on Arch Linux) when using the engineering-book theme.

  TeX Live remains an optional compatibility dependency for raw LaTeX
  projects that require `latexmkrc`, Biber, specialized TeX utilities, or a
  shell-escape workflow that has not been validated with Tectonic. Install
  XeLaTeX, latexmk, and the packages needed by that project in those cases.

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

OmniDoc keeps the most common lifecycle commands at the top level (`new`,
`init`, `build`, `watch`, `publish`, `status`, `open`, and `clean`) and groups
related tools by workflow:

```bash
omnidoc check --help       # diagnostics, validation, dependencies, locks, CI
omnidoc convert --help     # standalone Markdown conversion
omnidoc template --help    # template discovery and validation
omnidoc plugin --help      # plugin discovery and validation
omnidoc lib --help         # managed library lifecycle
```

The previous flat forms such as `config-validate`, `md2pdf`, `md2html`,
`list`, and `template --validate` remain available for script compatibility.
Project-aware commands discover the nearest `.omnidoc.toml` automatically, so
they can be run from the project root or nested directories such as
`chapters/drafts/`. An explicit `PATH` may likewise point anywhere inside the
project.

Commands with `--json` write only JSON to stdout and use a non-zero exit code
on failure. The `new`, `status`, `clean`, `update`, `config show`, and
`config get` responses include a `schema_version` field set to `1`. Their
command-specific success objects can be consumed directly, while failures return
`{"schema_version":1,"error":{"category":"...","message":"..."}}` and write
the human-readable diagnostic to stderr. Update reports additionally expose
`ready`, structured repository changes, exact actions, and optional per-action
unified diffs.

### Quick Start

1. **Create a new documentation repository**

   ```bash
   omnidoc new <PATH> [--title "Document Title"] [--author "Author Name"]
   ```

   The title defaults to the directory name, so the shortest useful form is:

   ```bash
   omnidoc new hello
   ```

   Use `--type` for a direct, scriptable creation path, or `--defaults` to use
   the recommended `ctex-md` template without prompting:

   ```bash
   omnidoc new hello --type ctex-md --author "John Doe"
   omnidoc new hello --defaults
   omnidoc new report --format latex  # only show LaTeX templates in the selector
   omnidoc new hello --type ctex-md --dry-run
   omnidoc new hello --type ctex-md --dry-run --json
   omnidoc new hello --type ctex-md --no-commit
   ```

   Without `--type` or `--defaults`, OmniDoc shows a searchable selector for
   built-in and external templates. In non-interactive environments it exits
   with guidance instead of attempting to open a prompt or leaving a partial
   directory behind.

   Example (inquire-based selection):

   ```
   $ omnidoc new hello

   ? Select project template:
   > markdown ctex-md — ctex class based markdown document writing system [built-in]
     markdown ebook-md — elegantbook class based markdown document writing system [built-in]
     latex    ctexart-tex — raw ctexart document type [built-in]

   [Type to filter, use arrow keys to navigate, Enter to confirm, Esc/Ctrl+C to cancel]
   ```

   Run `omnidoc template list` (or add `--json`) to inspect every accepted
   template key before creating a project.

   `--dry-run` resolves the target path, inferred title, author, template,
   directories, files, Git initialization, and initial commit without creating
   the target directory. Add `--json` for a stable action report suitable for
   scripts. A regular creation initializes Git and creates one content-bearing
   `Create project` commit; `--no-commit` leaves the generated files in an
   unborn repository for users who want to choose their own first commit.

   After selecting a template, the tool creates the repository with the following structure:

   ```
   biblio/     # Bibliography files (.bib)
   dac/        # D2 diagram source files
   drawio/     # Draw.io diagram source files
   figure/     # Third-party/static figure assets
   figures/    # Generated figure output directory
   md/         # Additional markdown files (for markdown projects)
   tex/        # Additional LaTeX files (for LaTeX projects, if configured)
   main.md     # Main entry file (or main.tex for LaTeX projects)
   ```

   The project is automatically initialized as a Git repository. If the target
   path already exists, `new` leaves it untouched and points to `omnidoc init`
   instead.

2. **Initialize an existing repository**

   If you have an existing directory with markdown or LaTeX files, you can initialize it as an omnidoc project:

   ```bash
   omnidoc init [PATH] [--title "Document Title"] [--author "Author Name"]
   omnidoc init existing-repo --type ctex-md
   omnidoc init existing-repo --defaults
   omnidoc init existing-repo --type ctex-md --no-commit
   ```

   If `PATH` is not specified, the current directory is used. The tool will:
   - Infer the title from the directory name when `--title` is omitted
   - Prompt for a template, or use `--type`/`--defaults` non-interactively
   - Move existing `.md` and `.tex` files to appropriate directories
   - Create the directory structure
   - Initialize git repository if not already present

   When an existing Git repository already has staged, modified, deleted, or
   untracked files, `init` refuses to include them in its automatic commit. Commit
   or stash that work first, or use `--no-commit` to initialize OmniDoc files
   while leaving all Git decisions to the user.

3. **Build the repository**

   Build your content. Markdown projects can output `pdf`, `html`, `epub`, `docx`, `pptx`, or `latex`; LaTeX projects output PDF.

   ```bash
   omnidoc build [PATH] [--to <FORMAT>] [--output <FORMAT>]... [--all] [OPTIONS]
   ```

   - If `PATH` is not specified, the current directory is used
   - Use `--to html`, `--to epub`, `--to docx`, `--to pptx`, or `--to latex` for Markdown project builds
   - Use repeated `--output <FORMAT>` to build a specific set of outputs
   - Use `--all` to build `[build].outputs` or the default set: `pdf`, `html`, `docx`, `epub`
   - Markdown PDF engine order is bundled Tectonic, Tectonic from `PATH`, then XeLaTeX
   - Raw LaTeX projects prefer XeLaTeX + latexmk and fall back to Tectonic only when XeLaTeX is unavailable
   - Use `--pdf-engine xelatex` or `--pdf-engine tectonic` to force a specific engine
   - Use `--latex-backend engine --max-latex-passes 5` for direct XeLaTeX/LuaLaTeX/PDFLaTeX builds that stop when `.aux/.toc`-style files stop changing
   - Keep the default `--latex-backend latexmk` when you need bibliography/glossary automation or custom `.latexmkrc` rules
   - Use `--force` to ignore the `.omnidoc-cache` input/config hash and rebuild
   - Use `--report` to write `build/omnidoc-report.json`
   - Use `--write-lock` to update `omnidoc.lock` after a successful build
   - Use `--strict` to fail on lint/config warnings before building
   - Use `--verbose` to show detailed build messages
   - The build directory is `build/` (configurable via config), and output files use `[project].target` or fall back to the repository directory name

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
   omnidoc build --to pptx
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
   # Omit latex_engine (or use "auto") for the managed engine policy.
   latex_engine = "auto"
   # tectonic = "/custom/path/to/tectonic"

   [tectonic]
   # Optional URL or local Tectonic bundle for controlled/offline builds.
   # bundle = "./vendor/tectonic-bundle.tar"
   only_cached = false
   shell_escape = false
   search_paths = ["./tex", "./biblio"]

   [build]
   outputs = ["pdf", "html", "docx"]
   latex_backend = "engine"
   max_latex_passes = 5

   [theme]
   name = "engineering-book"
   version = "1"
   compatibility = "readium"

   [pandoc]
   toc = true
   css = "styles/manual.css"
   html_template = "templates/page.html"
   latex_template = "templates/report.tex"
   epub_template = "templates/book.html"
   reference_doc = "templates/reference.docx"
   pptx_reference_doc = "templates/reference.pptx"
   epub_css = "styles/epub.css"

   [pandoc.format_options]
   # Writer-specific options are appended after common `pandoc.options`.
   # This is useful when the same setting has different semantics by writer.
   epub = ["--toc-depth=3"]
   pdf = ["--toc-depth=1"]
   ```

   `template` is still accepted as a generic fallback for template-capable outputs. DOCX uses `reference_doc` instead of Pandoc `--template`.

   `tectonic.only_cached = true` disables network downloads. Use it only after
   prewarming Tectonic's cache or configuring a complete local `bundle`.
   `tectonic.shell_escape` is off by default because Tectonic exposes it as an
   unstable capability; enabling it is an explicit trust decision for the
   current project. OmniDoc translates the bundled `texmf` tree, configured
   search roots, `TEXINPUTS`, `BIBINPUTS`, and `TEXMFHOME` into Tectonic search
   paths and records local files consumed by Tectonic in
   `.omnidoc-cache/latex-inputs.d`.

   Tectonic is XeTeX-compatible and has been validated here with Unicode,
   English, CJK, OpenType fonts, xeCJK, TikZ, tcolorbox, tables, math, emoji,
   automatic reruns, and BibTeX-style builds. It is not a complete behavioral
   replacement for XeLaTeX + latexmk: it does not read `latexmkrc`, Biber still
   needs an external orchestration path, and shell-escape compatibility is not
   universal. For those projects, keep the raw LaTeX default and install TeX
   Live.

   The rationale, tested asset choices, compatibility boundary, and rollback
   are recorded in
   [ADR 0001](docs/decisions/0001-tectonic-engine-policy.md).

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

   `watch` uses the native `notify` backend, rebuilds once immediately, then
   rebuilds on source changes such as `.md`, `.tex`, `.bib`, `.drawio`, `.dot`,
   `.json`, and common image assets. It ignores the configured build output,
   `dist/`, cache directories, generated figure output, and exact generated
   artifacts when the output directory is the project root, preventing build
   products from triggering a rebuild loop. Build failures are printed and the
   watcher keeps running. With `--once`, the initial build result is returned
   directly, including a non-zero exit status on failure. There is no polling
   fallback.

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

6. **Inspect project status and artifacts**

   Resolve the effective entry file, source format, target, output directory,
   configured formats, and whether each expected artifact exists:

   ```bash
   omnidoc status [PATH]
   omnidoc status [PATH] --json
   ```

   This is a read-only command and is useful both after project creation and
   in scripts that need to discover artifact paths without reproducing
   OmniDoc's configuration rules.

7. **Open a built document**

   ```bash
   omnidoc open [PATH] [--to <FORMAT>]
   omnidoc open [PATH] [--to <FORMAT>] --print-path
   ```

   Without `--to`, OmniDoc opens the configured `[project].to` artifact. It
   supports PDF, HTML, EPUB, DOCX, PPTX, and generated LaTeX files, honors the
   configured target and output directory, and uses the platform's default
   viewer. `--print-path` validates the artifact and prints its absolute path
   without launching a GUI.

8. **Clean the repository safely**

   Preview or remove build artifacts:

   ```bash
   omnidoc clean [PATH] --dry-run
   omnidoc clean [PATH] --dry-run --json
   omnidoc clean [PATH]
   omnidoc clean [PATH] --distclean
   ```

   - `--dry-run` reports every file/directory target, file count, and byte size without modifying the project
   - `clean` removes the configured build directory; if the output directory is the project root, only known artifacts and temporary files are removed
   - `clean --distclean` additionally removes known target/entry LaTeX temporary files and the generated `auto/` directory
   - Cleaning refuses absolute output directories or paths containing `..`, and never removes unrelated root-level PDFs

### Project Management Commands

9. **Update a document repository**

   Preview or refresh an existing OmniDoc project structure:

   ```bash
   omnidoc update [PATH] --dry-run
   omnidoc update [PATH] --diff
   omnidoc update [PATH] --dry-run --json
   omnidoc update [PATH]
   omnidoc update [PATH] --no-commit
   ```

   The preview lists only real managed-file changes, directory creation, source
   moves, Git initialization, and commits without taking a project lock or
   creating `.omnidoc-cache/`. `--diff` implies preview mode and includes a
   unified diff for `.gitignore`, `figure/README.md`, and `.latexmkrc` changes;
   JSON attaches that diff to the corresponding action.

   A regular update writes managed files atomically and creates a commit only
   when Git-visible content changed. If a repository with existing commits is
   already dirty, OmniDoc refuses to mix those unrelated changes into its
   automatic commit. Commit or stash them first, or use `--no-commit`. Repositories
   without a first commit can still be initialized normally. Root-level Markdown
   and LaTeX sources are moved to their resolved source directories, and tracked
   moves correctly record both deletion and addition. Destination collisions
   abort the entire update before any file is changed.

10. **List all project templates**

   Preview available built-in types and external templates:

   ```bash
   omnidoc template list
   omnidoc template list --format markdown
   omnidoc template list --json
   ```

   This displays the key, format, source, entry filename, and description for
   every template accepted by `new --type` and `init --type`. The legacy
   `omnidoc list` form is still supported.

### Configuration Commands

11. **Inspect or create configuration**

   Inspect the effective configuration, read one value, or explicitly create
   the user-level configuration file:

   ```bash
   omnidoc config show [PATH]
   omnidoc config show [PATH] --scope merged --json
   omnidoc config show --scope global --json
   omnidoc config get target [PATH]
   omnidoc config get project.target [PATH] --scope project --json
   omnidoc config init --author "Author Name" [OPTIONS]
   ```

   `show` and `get` are read-only: on first run they use in-memory defaults and
   do not create a config directory or `omnidoc.toml`. The default `merged`
   scope combines user and project configuration and reports every source path;
   `global` and `project` expose the original schema for that file. Dot-separated
   keys let scripts read exact values without parsing the whole configuration.

   `config init` starts from the same complete first-run defaults used in
   memory, then overrides only explicitly supplied values. Its options are:
   - `--author <AUTHOR>`: Configure the author name (required)
   - `--lib <LIB>`: Configure the OmniDoc library path
   - `--outdir <OUTDIR>`: Configure the output directory for building (default: `build`)
   - `--texmfhome <TEXMFHOME>`: Configure the TEXMFHOME environment variable
   - `--bibinputs <BIBINPUTS>`: Configure the BIBINPUTS environment variable
   - `--texinputs <TEXINPUTS>`: Configure the TEXINPUTS environment variable
   - `--force`: Force generation (overwrite existing config)

   Example:
   ```bash
   omnidoc config init --author "John Doe" --outdir "output" --lib "$HOME/.local/share/omnidoc"
   ```

   The legacy `omnidoc config --authors NAME ...` form remains accepted for
   existing scripts but is hidden from the primary help surface.

12. **Maintain the OmniDoc library**

   Install, update, inspect, or verify the OmniDoc library files (`libs` is a
   visible alias of `lib`):

   ```bash
   omnidoc lib install         # Install the archive bound to this OmniDoc release
   omnidoc lib update          # Verify and replace from the same release channel
   omnidoc lib status          # Show version, release and compatibility
   omnidoc lib verify          # Verify required files and every SHA-256 entry
   omnidoc lib verify --json
   ```

   `libs` remains an alias of `lib`, and the previous flag forms such as
   `omnidoc lib --verify` remain accepted for scripts. Install and update fail
   if the downloaded manifest, compatibility contract,
   required resources, or payload checksums do not verify. OmniDoc and the
   library bundle always share one version and one GitHub release. Install and
   update read the embedded `omnidoc-libs.toml` contract, download the matching
   `.tar.gz` and external SHA-256 file, reject unsafe archive entries, verify the
   internal manifest/checksums, and promote the extracted bundle transactionally.
   A custom `[lib].path` can select a local bundle such as `bundles/libs` for
   development; Git clone installation is no longer supported.

13. **Inspect versioned theme bundles**

   Theme bundles are declared by `themes/<name>.toml` inside the installed
   OmniDoc library bundle and
   can bind matching HTML CSS, EPUB CSS, LaTeX packages, PPTX reference decks,
   Lua filters, font
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
   pptx_reference_doc = "pandoc/data/reference-docs/engineering-slides.pptx"
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

   Theme metadata defaults are applied by a managed Lua filter only when the
   document did not already define the same key. Front matter, an authoritative
   `build.metadata_file`, explicit `pandoc.lang`, and user-supplied Pandoc
   options therefore retain priority. The build target remains an artifact
   filename and the global author remains a template-creation default; neither
   replaces publication title/author metadata. Metadata keys are validated as
   portable scalar identifiers and values must remain single-line strings.

   `html_template`, `epub_template`, and `latex_template` bind a template to a
   specific writer. Explicit project format templates and the generic
   `pandoc.template` override the theme. When no PDF/LaTeX template is
   selected, OmniDoc deliberately lets Pandoc use the version-matched built-in
   template (the same template printed by `pandoc -D latex`). Book styling is
   layered through theme headers and `.sty` packages, so routine Pandoc
   upgrades do not require copying and rebasing the full upstream template.
   `pptx_reference_doc` binds a Pandoc reference presentation to the theme;
   project-level `pandoc.pptx_reference_doc` (or the legacy shared
   `pandoc.reference_doc`) takes precedence.

### Project Quality and CI Commands

Run environment diagnostics:

```bash
omnidoc check doctor [PATH]
omnidoc check doctor --json
omnidoc check doctor --strict [PATH]
omnidoc check doctor --strict --output html [PATH]
```

`omnidoc doctor ...` remains a top-level shortcut for this frequently used
diagnostic.

`doctor` derives its checks from the configured entry format and outputs. It
reports the resolved executable and version for required Pandoc, cross-reference,
LaTeX, and EPUB tools; verifies the omnidoc-libs manifest, checksums, and version
compatibility; validates the selected theme resources; and, for PDF themes,
checks declared fonts and system LaTeX packages. JSON output preserves failed
checks as structured results so it can be consumed by CI and support tooling.
Use `--strict` to return a non-zero status when any diagnostic fails; without
it, `doctor` retains its informational exit behavior.
Repeat `--output` to diagnose only the formats a particular build invocation
will produce instead of every output configured by the project.
Explicit executable paths can be configured under `[tools]`, including
`pandoc`, `pandoc_crossref`, `latex_engine`, `latexmk`, and `epubcheck`.

Validate project configuration:

```bash
omnidoc check config [PATH]
```

Lint source references and configured resources:

```bash
omnidoc check lint [PATH] [--strict]
```

Inspect the tracked dependency graph used by cache, reports, and lock files:

```bash
omnidoc check deps [PATH]
omnidoc check deps --json
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
omnidoc check lock [PATH]
omnidoc check lock --update
omnidoc check lock --check
```

`lock --check` exits with an error when `omnidoc.lock` is missing or stale.
Lock schema v4 uses BLAKE3 content digests and stores dependencies and resolved
resources separately for every configured output target. It also records the
selected OmniDoc library release/content digest and detected Pandoc,
pandoc-crossref, LaTeX engine, and PDF theme font identities. Font identities
include the resolved family, style, font version, file name, and content digest.
Toolchain identities now participate in cache keys, so replacing Pandoc,
XeLaTeX, or a font invalidates existing outputs. When the library bundle
provides a manifest, its declared version plus manifest/checksum digests are
locked as well. Older lock files must be regenerated with
`omnidoc check lock --update`.
Lock, cache, and build-report files are replaced atomically. Mutating build and
lock-update commands also hold `.omnidoc-cache/project.lock`, so
two OmniDoc processes cannot concurrently publish inconsistent project state.

For XeLaTeX, pdfLaTeX, and LuaLaTeX PDF builds, OmniDoc also enables the TeX
recorder and converts the engine's `.fls` file into
`.omnidoc-cache/latex-inputs.d`. Project `\input` files and indirect system
resources actually loaded by the engine are content-hashed on later cache and
lock checks. The first successful build adopts the depfile before writing its
cache entry, so it does not require a second build to stabilize.

Run CI-mode validation and builds:

```bash
omnidoc check ci [PATH] [--output pdf] [--output html]
```

`check ci` runs strict validation, builds all configured/default outputs,
writes `build/omnidoc-report.json`, and updates `omnidoc.lock`. The legacy flat
quality commands remain supported for existing automation.

Run the real Pandoc Golden Book gate locally before release-oriented changes:

```bash
scripts/check-golden-book.sh
scripts/check-golden-pdf.sh
```

Scheduled and manually dispatched CI also records cold and cached timings for
a generated 100-chapter/1,000-section benchmark. Run the same workload locally:

```bash
scripts/benchmark-large-book.sh
```

The PDF gate also renders every page at a fixed DPI and checks the committed
page-aware visual contract (page count, canvas, ink bounds/coverage, and a
perceptual hash). Nightly and release CI retain the PDF, rendered PGM pages,
font inventory, extracted text, and JSON comparison report as a diagnostic
artifact. After an intentional layout change, review the render and refresh
the contract explicitly with:

```bash
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
macOS and Windows jobs additionally install Pandoc and pandoc-crossref and build
a real HTML fixture. Every packaged archive is extracted and its contained
binary and release contract are smoke-tested. Version-tag workflows build the
release-bound library archive from `bundles/libs`, install that local candidate
through the same verified archive path used by end users, run `doctor --strict`,
and build the Golden Book with the packaged OmniDoc binary before publishing
the GitHub release.

Official OmniDoc archives and Debian packages include
`omnidoc-libs.toml`, a machine-readable release contract declaring the matching
library version/tag, release archive URL, and external SHA-256 URL. CI requires
the Cargo package, embedded release contract, and `bundles/libs/manifest.toml`
to use the same version before packaging. Verify it locally with:

```bash
python3 scripts/check-library-contract.py
```

The ordered tag, release-archive, packaged-install, and EPUB reader acceptance
procedure is maintained in [`release/CHECKLIST.md`](release/CHECKLIST.md).

List discovered local plugins and external template manifests:

```bash
omnidoc plugin list [PATH]
omnidoc plugin list --json
omnidoc plugin validate [PATH]
omnidoc plugin validate --json
```

`plugin validate` parses discovered `manifest.toml` files and checks template
plugin fields such as `language` and `template_file`. `plugin list --json` and
`plugin validate --json` also report declared hooks, and validation checks local
hook command paths when the command contains a path separator. The previous
flat `omnidoc plugin [PATH] --validate` form remains supported for scripts.
Plugin manifests use schema version 1 and may declare their OmniDoc compatibility:

```toml
manifest_version = 1
key = "project-lint"
version = "1.0.0"
compatible_omnidoc = ">=1.3.0,<2.0.0"
```

For compatibility with existing local plugins, an omitted `manifest_version`
is interpreted as version 1. Unsupported schema versions, invalid compatibility
ranges, and plugins incompatible with the running OmniDoc version fail
`plugin --validate`. Hook processes receive `OMNIDOC_PLUGIN_MANIFEST_VERSION`.

### Document Formatting Commands

11. **Format documents**

    Format markdown or LaTeX documents recursively:

    ```bash
    omnidoc fmt [PATHS...] [OPTIONS]
    ```

    Options:

    - `--backup`: Create backup files before formatting
    - `--check`: Report files requiring formatting and return a non-zero status without writing
    - `--diff`: Print unified diffs and return a non-zero status without writing
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
    Writes use an atomic replacement and preserve UTF-8 BOM, CRLF/LF style,
    final-newline state, and Unix file mode. Files whose bytes are already
    formatted are not rewritten.

    Examples:

    ```bash
    omnidoc fmt main.md                    # Format a single file
    omnidoc fmt md/                        # Format all files in md directory
    omnidoc fmt --backup --semantic .      # Format all files in current directory with backup
    omnidoc fmt --check .                   # CI-safe formatting gate
    omnidoc fmt --diff main.md              # Review changes without writing
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

      The same JSON can be written directly in Markdown. OmniDoc's default
      diagram filter renders fenced `bitfield` blocks as SVG for HTML, EPUB,
      DOCX, and PPTX outputs, and as PDF for PDF/LaTeX outputs:

      ````markdown
      ```{.bitfield #fig-control-word caption="SASS control word" width="100%"}
      {
        "bits": 21,
        "hspace": 840,
        "vspace": 92,
        "fontsize": 16,
        "entries": [
          { "name": "stall", "bits": 4, "attr": "(4)" },
          { "name": "Y", "bits": 1 },
          { "name": "write", "bits": 3, "attr": "barr" },
          { "name": "read", "bits": 3, "attr": "barr" },
          { "name": "wait mask", "bits": 6, "attr": "(6)" },
          { "name": "reuse", "bits": 4, "attr": "(4)" }
        ]
      }
      ```
      ````

      Entries are listed from least-significant to most-significant field. The
      code block accepts the same document-level renderer options as JSON files,
      plus Pandoc figure attributes such as `caption`, `width`, and an identifier.

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

    - **Export KiCad schematics**

      ```bash
      omnidoc figure kicad <SOURCES>... [OPTIONS]
      ```

      Options:
      - `--kicad-cli <KICAD_CLI>`: KiCad CLI executable path
      - `--format <FORMAT>`: `svg` or `pdf` (default: svg)
      - `--black-and-white`: Export print-friendly monochrome artwork
      - `--exclude-drawing-sheet`: Omit the KiCad title block and sheet border
      - `--pages <PAGES>`: Export selected comma-separated page numbers

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

    # Export a publication-ready KiCad schematic
    omnidoc figure kicad schematics/amplifier.kicad_sch \
      --format svg --exclude-drawing-sheet

    # Generate all figures in a directory
    omnidoc figure drawio/ --format pdf --output figure/
    ```

    Markdown projects also support semantic containers, source inclusion, and
    native diagram fenced blocks. The complete public syntax is documented in
    the installed `BLOCKS.md`. Keep substantial circuit and SPICE sources in
    separate versioned files with `include-code`:

    ````markdown
    ```{.circuit #fig-amplifier include-code="schematics/amplifier.py"
    caption="Common-emitter amplifier" width="80%"}
    ```

    ```{.spiceplot #fig-response include-code="sim/amplifier-ac.json"
    caption="Small-signal frequency response" width="80%"}
    ```
    ````

    Circuit sources receive a configured Schemdraw drawing as `d` and
    `schemdraw.elements` as `elm`. Spiceplot JSON specifies `netlist`, an
    ngspice `analysis` command, and a `traces` array. OmniDoc renders PDF for
    PDF/LaTeX and SVG for HTML/EPUB, and records both source files and SPICE
    netlists in its dependency graph and lock digest.

### Document Conversion Commands

13. **Convert markdown to PDF**

    Convert markdown files directly to PDF without creating a full project:

    ```bash
    omnidoc convert pdf <INPUTS>... [OPTIONS]
    ```

    Options:
    - `--lang <LANG>`: Language (cn or en)
    - `--output <OUTPUT>`: Output file path

    Examples:
    ```bash
    omnidoc convert pdf document.md --lang cn --output document.pdf
    omnidoc convert pdf file1.md file2.md --output combined.pdf
    ```

14. **Convert markdown to HTML**

    Convert markdown files to HTML:

    ```bash
    omnidoc convert html <INPUTS>... [OPTIONS]
    ```

    Options:
    - `--output <OUTPUT>`: Output file path (for single input) or directory (for multiple inputs)
    - `--css <CSS>`: CSS file path for styling

    Examples:
    ```bash
    omnidoc convert html document.md --output document.html
    omnidoc convert html file1.md file2.md --output html/ --css style.css
    ```

    `md2pdf` and `md2html` remain accepted as legacy command forms.

### Template Management Commands

15. **Template toolkit**

    List templates or validate external template manifests and files:

    ```bash
    omnidoc template list
    omnidoc template list --json
    omnidoc template validate
    omnidoc template validate simple-md --json
    ```

    This command validates all external templates (hot-loaded, no restart needed). It checks:
    - Manifest parsing
    - Template file existence
    - Minimal Tera render with `title/author/date`

### Utility Commands

16. **Generate shell completion**

    Generate shell completion scripts for bash, zsh, fish, elvish, or PowerShell:

    ```bash
    omnidoc complete <SHELL>
    ```

    Supported shells: `bash`, `zsh`, `fish`, `elvish`, `powershell`

    Example:
    ```bash
    # For zsh
    omnidoc complete zsh > ~/.zsh_completions/_omnidoc

    # For bash
    omnidoc complete bash > ~/.bash_completion.d/omnidoc
    ```

    The previous `omnidoc complete --generate <SHELL>` form is still accepted.

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

`template_file` and `file_name` must be safe relative paths. Absolute paths and
`..` traversal are rejected by validation and cannot be used during project
creation. Nested entry paths such as `docs/index.md` are supported.

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
omnidoc template list
omnidoc template list --json
```

- Validate external templates (hot-loaded, no restart):

```bash
omnidoc template validate
omnidoc template validate simple-md --json
```

The validator checks manifest parsing, template file existence, and a minimal Tera render with `title/author/date`.

### Initialize with external templates

Select an external template interactively or pass its key directly:

```bash
omnidoc new notes --type simple-md
omnidoc init existing-notes --type simple-md
```

External template language and entry filename metadata now flow into generated
`.omnidoc.toml`, so custom Markdown and LaTeX templates work through the same
non-interactive creation path as built-in templates.
