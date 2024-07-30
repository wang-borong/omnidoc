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
        project: String, // it's also the doc name
        /// set root path to create the documentation project
        root: Option<String>,
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

        // create makefile
        //#[arg(long, default_value_t = false)]
        //makefile: bool,
    },

    /// init a project
    Init {
        /// path to documentation project
        #[arg(short, long)]
        path: String,
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
    },

    /// build the document project
    Build {
        /// path to documentation project
        path: Option<String>, 
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
        /// path to documentation project
        path: String,
    },
}

pub fn cli() -> Result<(), std::io::Error> {
    let args = OmniCli::parse();

    println!("{:?}", args);

    match args.command {
        Commands::Init { path, author, docver, release, language, suffix } => {
            // TODO: Use configuration file to set the default infos
            let _author = match author {
                Some(_author) => _author,
                None => "王伯榕".to_string(),
            };
            let _docver = match docver {
                Some(_docver) => _docver,
                None => "v0.1".to_string(),
            };
            let _release = match release {
                Some(_release) => _release,
                None => "v1.0".to_string(),
            };
            let _language = match language {
                Some(_language) => _language,
                None => "zh".to_string(),
            };
            let doc = Doc::new("", &path, &_author, &_docver, &_release, &_language);
            doc.init_project()?;
        },
        Commands::Build { path, output, builder, jobs } => {
            todo!();
        },
        Commands::Clean { path } => {
            let doc = Doc::new("", &path, "", "", "", "");
        },
        Commands::Create { project, root, author, docver, release, language, suffix } => {
            let _root = match root {
                Some(_root) => _root,
                None => "./".to_string(),
            };
            let _author = match author {
                Some(_author) => _author,
                None => "王伯榕".to_string(),
            };
            let _docver = match docver {
                Some(_docver) => _docver,
                None => "v0.1".to_string(),
            };
            let _release = match release {
                Some(_release) => _release,
                None => "v1.0".to_string(),
            };
            let _language = match language {
                Some(_language) => _language,
                None => "zh".to_string(),
            };
            let doc = Doc::new(&project, &_root, &_author, &_docver, &_release, &_language);
            doc.create_project()?;
        }
    }

    Ok(())
}

