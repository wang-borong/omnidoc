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

   When you use this command to create a new repository, you'll be prompted to select a documentation template in the omnidoc command line.
   You can press "Tab" key twice to see the supported documentation types.

   ```
   doctype> 
   ctart-tex      ctexart-tex    ctexrep-tex    ebook-md       enote-md       moderncv-tex
   ctbook-tex     ctexbook-tex   ctrep-tex      ebook-tex      enote-tex      resume-ng-tex
   ```

   The suffixes `-tex` and `-md` indicate the text format.

   After selecting a documentation type, the tool will create the repository for you.

   ```
   biblio  dac  drawio  figure  figures  main.md  md
   ```

   With the documentation writing system, you can use draw.io, Graphviz, and D2 to create diagrams and figures. It also supports converting figure formats with Inkscape and ImageMagick. To use these tools, make sure they are installed beforehand.

2. Initialize an existing repository

   The initialization function of omnidoc works similarly to the "new" function.

   ```bash
   omnidoc init --title "hello"
   ```

   The remaining steps are the same as when creating a new repository.

3. Build the repository

   Once you have written some content, you can build it into a PDF file to review.

   ```bash
   omnidoc build
   ```

   The build directory is located in the repository as 'build', and the PDF file will be named after the repository directory.

4. Clean the repository

   You can clean the repository at any time.

   ```bash
   omnidoc clean [--distclean]
   ```
