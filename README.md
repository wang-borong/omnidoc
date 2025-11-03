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

- **Draw.io**  

  Download Draw.io from its [GitHub releases](https://github.com/jgraph/drawio-desktop/releases).

- **Graphviz**  

  Install it through your Linux distribution's package manager.

- **Inkscape**  

  Install it through your Linux distribution's package manager.

- **ImageMagick**  

  Install it through your Linux distribution's package manager.

## Usage

1. Create a new documentation repository

   ```bash
   omnidoc new hello --title "hello"
   ```

   You'll be prompted to choose a template (built-in + external) with an interactive selector. Use arrow keys to navigate, Enter to select. You can also type an external template key directly (e.g., `simple-md`).

   To preview available types/templates at any time:

   ```bash
   omnidoc list
   ```

   Example (inquire-based selection):

   ```
   $ omnidoc new hello --title "hello"

   ? 请选择文档类型:
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

   [上下键选择，Enter 确认，Esc/Ctrl+C 取消]
   ```

   The suffixes `-tex` and `-md` indicate the text format for built-in types; external templates are shown under “External templates”.

   After selecting a template, the tool creates the repository:

   ```
   biblio  dac  drawio  figure  figures  main.md  md
   ```

   You can use draw.io, Graphviz, and D2 to create diagrams and figures. The tool can also convert figure formats with Inkscape and ImageMagick. Please ensure these tools are installed beforehand.

2. Initialize an existing repository

   Initialization works similarly to "new" and supports the same template selection (including external templates):

   ```bash
   omnidoc init --title "hello"
   ```

3. Build the repository

   Build your content into a PDF for review:

   ```bash
   omnidoc build
   ```

   The build directory is `build/`, and the PDF file is named after the repository directory.

4. Clean the repository

   ```bash
   omnidoc clean [--distclean]
   ```

5. Open the built PDF document

   ```bash
   omnidoc open
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

