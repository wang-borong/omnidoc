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
}
