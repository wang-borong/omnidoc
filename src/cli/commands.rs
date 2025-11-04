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

        /// show verbose message
        #[arg(short = 'v', long)]
        verbose: bool,
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
    Lib {
        /// install the OmniDoc library to XDG_DATA_DIR
        #[arg(short, long)]
        install: bool,

        /// update the OmniDoc library
        #[arg(short, long)]
        update: bool,
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
