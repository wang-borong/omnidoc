# omnidoc

This is a wrapper, based on Pandoc and LaTeX, for a documentation writing system that helps manage document repositories.
With omnidoc, you can write in Pandoc markdown or LaTeX and convert these files to PDF easily.
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

   Build your content into a PDF for review:

   ```bash
   omnidoc build [PATH] [--verbose]
   ```

   - If `PATH` is not specified, the current directory is used
   - Use `--verbose` to show detailed build messages
   - The build directory is `build/` (configurable via config), and the PDF file is named after the repository directory

4. **Open the built PDF document**

   ```bash
   omnidoc open [PATH]
   ```

   Opens the built PDF document using the system's default PDF viewer.

5. **Clean the repository**

   Remove build artifacts:

   ```bash
   omnidoc clean [PATH] [--distclean]
   ```

   - `clean`: Removes the build directory
   - `clean --distclean`: Removes build directory and all generated files

### Project Management Commands

6. **Update a document repository**

   Update an existing omnidoc project structure:

   ```bash
   omnidoc update [PATH]
   ```

   This command updates the project structure, template files, and configuration to match the current omnidoc version.

7. **List all supported document types**

   Preview available built-in types and external templates:

   ```bash
   omnidoc list
   ```

   This displays all built-in document types and external templates that are available for selection.

### Configuration Commands

8. **Generate default configuration**

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

9. **Maintain the OmniDoc library**

   Install or update the OmniDoc library files:

   ```bash
   omnidoc lib --install    # Install library to XDG_DATA_DIR
   omnidoc lib --update     # Update the library
   ```

   The library contains templates, LaTeX classes, and other resources used by omnidoc.

### Document Formatting Commands

10. **Format documents**

    Format markdown or LaTeX documents recursively:

    ```bash
    omnidoc fmt [PATHS...] [OPTIONS]
    ```

    Options:
    - `--backup`: Create backup files before formatting
    - `--semantic`: Enable semantic formatting
    - `--symbol`: Enable symbol formatting (Chinese punctuation)

    Examples:
    ```bash
    omnidoc fmt main.md                    # Format a single file
    omnidoc fmt md/                        # Format all files in md directory
    omnidoc fmt --backup --semantic .      # Format all files in current directory with backup
    ```

### Figure Generation Commands

11. **Generate figures from source files**

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

12. **Convert markdown to PDF**

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

13. **Convert markdown to HTML**

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

14. **Template toolkit**

    Validate external template manifests and files:

    ```bash
    omnidoc template --validate
    ```

    This command validates all external templates (hot-loaded, no restart needed). It checks:
    - Manifest parsing
    - Template file existence
    - Minimal Tera render with `title/author/date`

### Utility Commands

15. **Generate shell completion**

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

