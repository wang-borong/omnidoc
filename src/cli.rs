use clap::{Parser, Subcommand};
use omnidoc::doc::Doc;


//
// Create a git-like cli program to manage our document project.
//

/// the omnidoc management cli
#[derive(Debug, Parser)]
#[command(name = "omnidoc")]
#[command(version, about = "the omnidoc management cli", long_about = None)]
struct OmniCli {
    /// document management subcommands
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// create a new project
    Create {
        /// set project name
        #[arg(short, long)]
        project: String,
        /// set author name
        #[arg(short, long)]
        author: Option<String>,
        /// set document version
        #[arg(short, long)]
        docver: Option<String>,
        /// set release name
        #[arg(short, long)]
        release: Option<String>,
        /// set language
        #[arg(short, long)]
        language: Option<String>,
        /// set suffix
        #[arg(short, long)]
        suffix: Option<String>,

        /// create makefile
        #[arg(long, default_value_t = false)]
        makefile: bool,
    },

    /// build the document project
    Build {
        /// path to documentation source files
        source: Option<String>, 
        /// path to output directory
        output: Option<String>,

        /// builder to use (default to 'pdf')
        #[arg(short, long)]
        builder: Option<String>,
        /// run in parallel with N processes, when possible.
        #[arg(short, long, value_name = "N")]
        jobs: Option<u8>,
    },

    /// clean the document project
    Clean {
        project: Option<String>,
    },
}

pub fn cli() {
    let args = OmniCli::parse();

    println!("{:?}", args);
}

