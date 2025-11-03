use clap::{Parser, Subcommand, ValueHint};

/// the omnidoc management cli
#[derive(Debug, Parser)]
#[command(name = "omnidoc")]
#[command(version, about = "the omnidoc management cli", long_about = None)]
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
        /// config the author name
        #[arg(short, long)]
        authors: String,
        /// config the omindoc lib path
        #[arg(short, long)]
        lib: Option<String>,
        /// config the output directory for building
        #[arg(short, long)]
        outdir: Option<String>,
        /// config the TEXMFHOME env (in which does the system find the texmf home)
        #[arg(short = 'T', long)]
        texmfhome: Option<String>,
        /// config the BIBINPUTS env (in which does the system find the bibliograpies)
        #[arg(short, long)]
        bibinputs: Option<String>,
        /// config the TEXINPUTS env (in which does the system find the tex sources)
        #[arg(short, long)]
        texinputs: Option<String>,

        /// force generation
        #[arg(short = 'F', long)]
        force: bool,
    },

    /// maintain the omnidoc library
    Lib {
        /// install the omnidoc lib to XDG_DATA_DIR
        #[arg(short, long)]
        install: bool,

        /// update the omnidoc lib
        #[arg(short, long)]
        update: bool,
    },

    /// list current supported document types
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
}
