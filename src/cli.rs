use clap::{Parser, Subcommand};
use git2::Repository;
use dirs::data_local_dir;

use omnidoc::doc::Doc;
use omnidoc::config::ConfigParser;
use omnidoc::git::git_pull;

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
    },

    /// build the document project
    Build {
        /// path to documentation project
        path: String, 
        /// path to output directory
        output: Option<String>,

        /// builder to use (default to 'pdf')
        #[arg(short, long)]
        builder: Option<String>,
    },

    /// clean the document project
    Clean {
        /// path to documentation project
        path: String,
    },

    /// generate configuration
    Config,

    /// omnidoc maintenance
    Lib {
        /// install omnidoc lib to XDG_DATA_DIR
        #[arg(short, long)]
        install: bool,

        /// update omnidoc lib
        #[arg(short, long)]
        update: bool,
    }
}

pub fn cli() {
    let args = OmniCli::parse();

    let config = ConfigParser::default();
    let author_conf = config.get_author_name();

    match args.command {
        Commands::Init { path, author, docver, release, language } => {
            let author = match author {
                Some(author) => author,
                None => {
                    match author_conf {
                        Ok(author) => author,
                        Err(_) => "Someone".to_string(),
                    }
                }
            };
            let docver = match docver {
                Some(docver) => docver,
                None => "v0.1".to_string(),
            };
            let release = match release {
                Some(release) => release,
                None => "v1.0".to_string(),
            };
            let language = match language {
                Some(language) => language,
                None => "zh".to_string(),
            };
            let doc = Doc::new("", &path, &author, &docver, &release, &language);
            match doc.init_project() {
                Ok(_) => { },
                Err(e) => { eprintln!("initial project failed ({})", e) },
            }
        },
        Commands::Build { path, output, builder } => {
            let doc = Doc::new("", &path, "", "", "", "");
            match doc.build_project(output, builder) {
                Ok(_) => { },
                Err(e) => { eprintln!("build project failed ({})", e) },
            }
        },
        Commands::Clean { path } => {
            let doc = Doc::new("", &path, "", "", "", "");
            match doc.clean_project() {
                Ok(_) => { },
                Err(e) => { eprintln!("clean project failed ({})", e) },
            }
        },
        Commands::Create { project, root, author, docver, release, language } => {
            let root = match root {
                Some(root) => root,
                None => "./".to_string(),
            };
            let author = match author {
                Some(author) => author,
                None => {
                    match author_conf {
                        Ok(author) => author,
                        Err(_) => "Someone".to_string(),
                    }
                }
            };
            let docver = match docver {
                Some(docver) => docver,
                None => "v0.1".to_string(),
            };
            let release = match release {
                Some(release) => release,
                None => "v1.0".to_string(),
            };
            let language = match language {
                Some(language) => language,
                None => "zh".to_string(),
            };
            let doc = Doc::new(&project, &root, &author, &docver, &release, &language);
            match doc.create_project() {
                Ok(_) => { },
                Err(e) => { eprintln!("create project failed ({})", e) },
            }
        }
        Commands::Config => {
            match config.gen() {
                Ok(_) => println!("generate configuration success"),
                Err(e)  => eprintln!("generate configuration failed: {}", e),
            }
        }
        Commands::Lib { install, update } => {
            let dld = data_local_dir().unwrap();
            let olib = dld.join("omnidoc");

            if install  {
                let _repo = match Repository::clone_recurse("https://github.com/wang-borong/omnidoc-libs", &olib) {
                    Ok(repo) => repo,
                    Err(e) => panic!("failed to clone {}", e),
                };

            } else if update {
                match git_pull(&olib) {
                    Ok(()) => println!("update {} success", olib.display()),
                    Err(e) => eprintln!("update {} failed {}", olib.display(), e),
                }
            }
        }
    }
}

