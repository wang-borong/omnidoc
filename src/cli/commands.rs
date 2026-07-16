use clap::{Parser, Subcommand, ValueHint};

/// The OmniDoc management CLI
#[derive(Debug, Parser)]
#[command(name = "omnidoc")]
#[command(version, about = "The OmniDoc management CLI", long_about = None)]
pub struct OmniCli {
    /// document management subcommands
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// create a new project
    New {
        /// set the author name
        #[arg(short, long)]
        author: Option<String>,

        /// set the document title
        #[arg(short = 't', long)]
        title: String,

        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: String,
    },

    /// init a project
    Init {
        /// set the author name
        #[arg(short, long)]
        author: Option<String>,

        /// set the document title
        #[arg(short = 't', long)]
        title: String,

        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,
    },

    /// build the document project
    Build {
        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,

        /// override output format (pdf, html, epub, docx, latex)
        #[arg(long)]
        to: Option<String>,

        /// build all configured or default outputs
        #[arg(long)]
        all: bool,

        /// build multiple output formats (repeatable)
        #[arg(long = "output")]
        outputs: Vec<String>,

        /// override PDF engine (xelatex, lualatex, pdflatex, tectonic, or executable path)
        #[arg(long = "pdf-engine")]
        pdf_engine: Option<String>,

        /// LaTeX project backend (latexmk or engine)
        #[arg(long = "latex-backend", default_value = "latexmk")]
        latex_backend: String,

        /// maximum direct LaTeX engine passes for --latex-backend engine
        #[arg(long = "max-latex-passes")]
        max_latex_passes: Option<usize>,

        /// force rebuild even when input cache is unchanged
        #[arg(short = 'F', long)]
        force: bool,

        /// write build/omnidoc-report.json
        #[arg(long)]
        report: bool,

        /// update omnidoc.lock after a successful build
        #[arg(long = "write-lock")]
        write_lock: bool,

        /// fail on lint warnings before build
        #[arg(long)]
        strict: bool,

        /// show verbose message
        #[arg(short = 'v', long)]
        verbose: bool,
    },

    /// watch a document project and rebuild on source changes
    Watch {
        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,

        /// override output format (pdf, html, epub, docx, latex)
        #[arg(long)]
        to: Option<String>,

        /// build all configured or default outputs
        #[arg(long)]
        all: bool,

        /// build multiple output formats (repeatable)
        #[arg(long = "output")]
        outputs: Vec<String>,

        /// override PDF engine (xelatex, lualatex, pdflatex, tectonic, or executable path)
        #[arg(long = "pdf-engine")]
        pdf_engine: Option<String>,

        /// LaTeX project backend (latexmk or engine)
        #[arg(long = "latex-backend", default_value = "latexmk")]
        latex_backend: String,

        /// maximum direct LaTeX engine passes for --latex-backend engine
        #[arg(long = "max-latex-passes")]
        max_latex_passes: Option<usize>,

        /// debounce interval in milliseconds
        #[arg(long = "debounce-ms", default_value_t = 250)]
        debounce_ms: u64,

        /// run the initial build and exit after one scan cycle
        #[arg(long)]
        once: bool,

        /// force rebuild even when input cache is unchanged
        #[arg(short = 'F', long)]
        force: bool,

        /// write build/omnidoc-report.json
        #[arg(long)]
        report: bool,

        /// fail on lint warnings before build
        #[arg(long)]
        strict: bool,

        /// show verbose build messages
        #[arg(short = 'v', long)]
        verbose: bool,
    },

    /// build and publish generated artifacts into a dist directory
    Publish {
        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,

        /// override output format (pdf, html, epub, docx, latex)
        #[arg(long)]
        to: Option<String>,

        /// build/publish all configured or default outputs
        #[arg(long)]
        all: bool,

        /// build/publish multiple output formats (repeatable)
        #[arg(long = "output")]
        outputs: Vec<String>,

        /// override PDF engine (xelatex, lualatex, pdflatex, tectonic, or executable path)
        #[arg(long = "pdf-engine")]
        pdf_engine: Option<String>,

        /// LaTeX project backend (latexmk or engine)
        #[arg(long = "latex-backend", default_value = "latexmk")]
        latex_backend: String,

        /// maximum direct LaTeX engine passes for --latex-backend engine
        #[arg(long = "max-latex-passes")]
        max_latex_passes: Option<usize>,

        /// publish directory
        #[arg(long = "dist-dir", default_value = "dist")]
        dist_dir: String,

        /// publish tag or release directory name
        #[arg(long)]
        tag: Option<String>,

        /// copy existing build artifacts without rebuilding first
        #[arg(long = "no-build")]
        no_build: bool,

        /// verify an existing published release instead of building or copying
        #[arg(long, requires = "tag")]
        verify: bool,

        /// emit publish verification results as JSON
        #[arg(long, requires = "verify")]
        json: bool,

        /// force rebuild even when input cache is unchanged
        #[arg(short = 'F', long)]
        force: bool,

        /// fail on lint warnings before build
        #[arg(long)]
        strict: bool,

        /// show verbose build messages
        #[arg(short = 'v', long)]
        verbose: bool,
    },

    /// diagnose local tools, configuration, and template library
    Doctor {
        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,

        /// emit JSON diagnostics
        #[arg(long)]
        json: bool,

        /// return a non-zero exit status when any check fails
        #[arg(long)]
        strict: bool,
    },

    /// validate OmniDoc configuration files
    ConfigValidate {
        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,
    },

    /// lint document sources for missing resources and weak references
    Lint {
        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,

        /// treat warnings as errors
        #[arg(long)]
        strict: bool,
    },

    /// print the tracked project dependency graph
    Deps {
        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,

        /// emit JSON dependency graph
        #[arg(long)]
        json: bool,
    },

    /// run strict CI checks and configured builds
    Ci {
        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,

        /// output format to build (repeatable)
        #[arg(long = "output")]
        outputs: Vec<String>,
    },

    /// create or update omnidoc.lock
    Lock {
        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,

        /// check whether omnidoc.lock matches current project inputs
        #[arg(long, conflicts_with = "update")]
        check: bool,

        /// rewrite the lock file
        #[arg(long)]
        update: bool,
    },

    /// list discovered local plugins and external templates
    Plugin {
        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,

        /// emit JSON plugin metadata
        #[arg(long)]
        json: bool,

        /// validate discovered plugin/template manifests
        #[arg(long)]
        validate: bool,
    },

    /// open the built doc
    Open {
        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,
    },

    /// clean the document project
    Clean {
        /// distclean the project
        #[arg(short = 'D', long)]
        distclean: bool,

        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,
    },

    /// update a doc repo
    Update {
        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,
    },

    /// generate a default configuration
    Config {
        /// configure the author name
        #[arg(short, long)]
        authors: String,
        /// configure the OmniDoc library path
        #[arg(short, long)]
        lib: Option<String>,
        /// configure the output directory for building
        #[arg(short, long)]
        outdir: Option<String>,
        /// configure the TEXMFHOME environment variable (the directory where the system finds the texmf home)
        #[arg(short = 'T', long)]
        texmfhome: Option<String>,
        /// configure the BIBINPUTS environment variable (the directory where the system finds the bibliographies)
        #[arg(short, long)]
        bibinputs: Option<String>,
        /// configure the TEXINPUTS environment variable (the directory where the system finds the tex sources)
        #[arg(short, long)]
        texinputs: Option<String>,

        /// force generation
        #[arg(short = 'F', long)]
        force: bool,
    },

    /// maintain the OmniDoc library
    #[command(visible_alias = "libs")]
    Lib {
        /// install the OmniDoc library to XDG_DATA_DIR
        #[arg(short, long, conflicts_with_all = ["update", "status", "verify"])]
        install: bool,

        /// update the OmniDoc library
        #[arg(short, long, conflicts_with_all = ["install", "status", "verify"])]
        update: bool,

        /// show installed library version, revision, compatibility, and integrity
        #[arg(long, conflicts_with_all = ["install", "update", "verify"])]
        status: bool,

        /// verify the installed manifest, required resources, and checksums
        #[arg(long, conflicts_with_all = ["install", "update", "status"])]
        verify: bool,

        /// emit status or verification details as JSON
        #[arg(long)]
        json: bool,

        /// install, update, or verify a specific library tag, branch, or commit
        #[arg(long, value_name = "REVISION", conflicts_with = "release")]
        revision: Option<String>,

        /// install or update from the release archive bound to this OmniDoc version
        #[arg(long, conflicts_with_all = ["status", "verify", "revision"])]
        release: bool,
    },

    /// discover and validate versioned theme bundles
    Theme {
        #[command(subcommand)]
        subcommand: ThemeSubcommand,
    },

    /// list all supported document types
    List,

    /// template toolkit
    Template {
        /// validate external template manifests & files
        #[arg(long)]
        validate: bool,
    },

    /// generate shell completion
    Complete {
        /// If provided, outputs the completion file for given shell
        #[arg(short, long = "generate", value_enum)]
        generator: Option<clap_complete::Shell>,
    },

    /// convert markdown files to PDF
    Md2pdf {
        /// language (cn or en)
        #[arg(short, long)]
        lang: Option<String>,
        /// output file path
        #[arg(short, long)]
        output: Option<String>,

        /// input markdown files
        #[arg(required = true, value_hint = ValueHint::FilePath)]
        inputs: Vec<String>,
    },

    /// convert markdown files to HTML
    Md2html {
        /// output file path (for single input) or directory (for multiple inputs)
        #[arg(short, long)]
        output: Option<String>,

        /// CSS file path
        #[arg(short, long)]
        css: Option<String>,

        /// input markdown files
        #[arg(required = true, value_hint = ValueHint::FilePath)]
        inputs: Vec<String>,
    },

    /// format documents (recursively format directories or format files)
    Fmt {
        /// create backup files
        #[arg(short, long)]
        backup: bool,

        /// enable semantic formatting
        #[arg(short, long)]
        semantic: bool,

        /// enable symbol formatting (Chinese punctuation)
        #[arg(short = 'y', long)]
        symbol: bool,

        /// paths to format (files or directories)
        #[arg(value_hint = ValueHint::AnyPath)]
        paths: Vec<String>,
    },

    /// generate figures from source files
    Figure {
        #[command(subcommand)]
        subcommand: Option<FigureSubcommand>,

        /// output format (pdf, png, svg, etc.)
        #[arg(short = 'f', long, default_value = "pdf")]
        format: String,

        /// force regenerate even if output exists
        #[arg(short = 'F', long)]
        force: bool,

        /// output directory
        #[arg(short = 'o', long)]
        output: Option<String>,

        /// source figure files (auto-detect type if no subcommand specified)
        #[arg(value_hint = ValueHint::FilePath)]
        sources: Vec<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum ThemeSubcommand {
    /// list installed theme bundles
    List {
        /// emit JSON theme metadata
        #[arg(long)]
        json: bool,
    },

    /// inspect one installed theme bundle
    Inspect {
        /// theme name
        name: String,

        /// emit JSON theme metadata
        #[arg(long)]
        json: bool,
    },

    /// validate one theme, or every installed theme when NAME is omitted
    Validate {
        /// optional theme name
        name: Option<String>,

        /// emit JSON validation results
        #[arg(long)]
        json: bool,

        /// verify required font families with fontconfig
        #[arg(long)]
        check_fonts: bool,

        /// verify required system LaTeX packages with kpsewhich
        #[arg(long)]
        check_latex: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum FigureSubcommand {
    /// generate bitfield diagrams from JSON files
    Bitfield {
        /// source JSON files
        #[arg(required = true, value_hint = ValueHint::FilePath)]
        sources: Vec<String>,

        /// vertical space
        #[arg(long)]
        vspace: Option<u32>,

        /// horizontal space
        #[arg(long)]
        hspace: Option<u32>,

        /// rectangle lanes
        #[arg(long)]
        lanes: Option<u32>,

        /// overall bitwidth
        #[arg(long)]
        bits: Option<u32>,

        /// font family
        #[arg(long, default_value = "sans-serif")]
        fontfamily: String,

        /// font weight
        #[arg(long, default_value = "normal")]
        fontweight: String,

        /// font size
        #[arg(long, default_value = "14")]
        fontsize: u32,

        /// stroke width
        #[arg(long, default_value = "1.0")]
        strokewidth: f32,

        /// beautify output
        #[arg(long)]
        beautify: bool,

        /// use json5 parser
        #[arg(long)]
        json5: bool,

        /// do not use json5 parser
        #[arg(long)]
        no_json5: bool,

        /// compact mode
        #[arg(long)]
        compact: bool,

        /// horizontal flip
        #[arg(long)]
        hflip: bool,

        /// vertical flip
        #[arg(long)]
        vflip: bool,

        /// trim long bitfield names (character width)
        #[arg(long)]
        trim: Option<f32>,

        /// uneven lanes
        #[arg(long)]
        uneven: bool,

        /// legend item (format: NAME:TYPE, can be used multiple times)
        #[arg(long)]
        legend: Vec<String>,

        /// output format (pdf, png, svg, etc.)
        #[arg(short = 'f', long, default_value = "svg")]
        format: String,

        /// force regenerate even if output exists
        #[arg(short = 'F', long)]
        force: bool,

        /// output directory
        #[arg(short = 'o', long)]
        output: Option<String>,
    },

    /// generate diagrams from drawio files
    Drawio {
        /// source drawio files
        #[arg(required = true, value_hint = ValueHint::FilePath)]
        sources: Vec<String>,

        /// drawio executable path
        #[arg(short = 'd', long)]
        drawio: Option<String>,

        /// output format (pdf, png, svg, etc.)
        #[arg(short = 'f', long, default_value = "pdf")]
        format: String,

        /// force regenerate even if output exists
        #[arg(short = 'F', long)]
        force: bool,

        /// output directory
        #[arg(short = 'o', long)]
        output: Option<String>,
    },

    /// generate diagrams from graphviz dot files
    Dot {
        /// source dot files
        #[arg(required = true, value_hint = ValueHint::FilePath)]
        sources: Vec<String>,

        /// graphviz dot executable path
        #[arg(short = 'g', long)]
        gradot: Option<String>,

        /// output format (pdf, png, svg, etc.)
        #[arg(short = 'f', long, default_value = "pdf")]
        format: String,

        /// force regenerate even if output exists
        #[arg(short = 'F', long)]
        force: bool,

        /// output directory
        #[arg(short = 'o', long)]
        output: Option<String>,
    },

    /// generate diagrams from plantuml files
    Plantuml {
        /// source plantuml files
        #[arg(required = true, value_hint = ValueHint::FilePath)]
        sources: Vec<String>,

        /// plantuml executable path or jar file path
        #[arg(short = 'p', long)]
        plantuml: Option<String>,

        /// output format (pdf, png, svg, etc.)
        #[arg(short = 'f', long, default_value = "png")]
        format: String,

        /// force regenerate even if output exists
        #[arg(short = 'F', long)]
        force: bool,

        /// output directory
        #[arg(short = 'o', long)]
        output: Option<String>,
    },

    /// convert images (SVG and other formats)
    Convert {
        /// source image files
        #[arg(required = true, value_hint = ValueHint::FilePath)]
        sources: Vec<String>,

        /// inkscape executable path
        #[arg(short = 'i', long)]
        inkscape: Option<String>,

        /// imagemagick executable path
        #[arg(short = 'm', long)]
        imagemagick: Option<String>,

        /// output format (pdf, png, svg, etc.)
        #[arg(short = 'f', long, default_value = "pdf")]
        format: String,

        /// force regenerate even if output exists
        #[arg(short = 'F', long)]
        force: bool,

        /// output directory
        #[arg(short = 'o', long)]
        output: Option<String>,
    },
}
