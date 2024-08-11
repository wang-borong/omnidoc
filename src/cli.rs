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
    New {
        /// set the author name
        #[arg(short, long)]
        author: Option<String>,
        /// set the document version
        #[arg(short, long)]
        version: Option<String>,
        /// set the release name
        #[arg(short, long)]
        release: Option<String>,
        /// set the language
        #[arg(short, long)]
        language: Option<String>,

        /// set the document title
        #[arg(short = 't', long)]
        title: String,
        /// set the output document file name
        #[arg(short, long)]
        name: String,
        /// select a document type
        #[arg(short = 'T', long)]
        doctype: String,

        /// set the path to a documentation project
        path: String,

        // create makefile
        //#[arg(long, default_value_t = false)]
        //makefile: bool,
    },

    /// init a project
    Init {
        /// set the author name
        #[arg(short, long)]
        author: Option<String>,
        /// set the document version
        #[arg(short, long)]
        version: Option<String>,
        /// set the release name
        #[arg(short, long)]
        release: Option<String>,
        /// set the language
        #[arg(short, long)]
        language: Option<String>,

        /// set the document title
        #[arg(short = 't', long)]
        title: String,
        /// set the output document file name
        #[arg(short, long)]
        name: String,
        /// select a document type
        #[arg(short = 'T', long)]
        doctype: String,

        /// set the path to a documentation project
        path: String,
    },

    /// build the document project
    Build {
        /// builder to use (default to 'pdf')
        #[arg(short, long)]
        builder: Option<String>,

        /// set the path to a documentation project
        path: Option<String>,
        /// set the output path
        output: Option<String>,

    },

    /// clean the document project
    Clean {
        /// distclean the project
        #[arg(short = 'D', long)]
        distclean: bool,

        /// set the path to a documentation project
        path: Option<String>,

    },

    /// update a doc repo
    Update {
        /// set the path to a documentation project
        path: Option<String>,

        /// set the output document file name
        #[arg(short, long)]
        name: Option<String>,
    },

    /// generate a default configuration
    Config,

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
}

pub fn cli() {
    let args = OmniCli::parse();

    let mut config = ConfigParser::default();

    match args.command {
        Commands::Init { path, author, version, release, language, title, name, doctype } => {
            config.parse();
            let author_conf = config.get_author_name();
            let author = match author {
                Some(author) => author,
                None => {
                    match author_conf {
                        Ok(author) => author,
                        Err(_) => "Someone".to_string(),
                    }
                }
            };
            let version = match version {
                Some(version) => version,
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
            let doc = Doc::new(&title, &path, &author, &version,
                &release, &language, &doctype, &name);
            match doc.init_project(false) {
                Ok(_) => { },
                Err(e) => { eprintln!("initial project failed ({})", e) },
            }
        },
        Commands::Build { path, output, builder } => {
            let doc: Doc;
            match path {
                Some(path) => doc = Doc::new("", &path, "", "", "", "", "", ""),
                None => doc = Doc::new("", ".", "", "", "", "", "", ""),
            };
            match doc.build_project(output, builder) {
                Ok(_) => { },
                Err(e) => { eprintln!("build project failed ({})", e) },
            }
        },
        Commands::Clean { path, distclean } => {
            let doc: Doc;
            match path {
                Some(path) => doc = Doc::new("", &path, "", "", "", "", "", ""),
                None => doc = Doc::new("", ".", "", "", "", "", "", ""),
            };

            match doc.clean_project(distclean) {
                Ok(_) => { },
                Err(e) => { eprintln!("clean project failed ({})", e) },
            }
        },
        Commands::New { path, author, version, release, language, title, name, doctype } => {
            config.parse();
            let author_conf = config.get_author_name();
            let author = match author {
                Some(author) => author,
                None => {
                    match author_conf {
                        Ok(author) => author,
                        Err(_) => "Someone".to_string(),
                    }
                }
            };
            let version = match version {
                Some(version) => version,
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
            let doc = Doc::new(&title, &path, &author, &version,
                &release, &language, &doctype, &name);
            match doc.create_project() {
                Ok(_) => { },
                Err(e) => { eprintln!("create project failed ({})", e) },
            }
        }
        Commands::Update { path, name } => {
            let mut doc: Doc;
            let has_name: bool;
            let name = match name {
                Some(name) => {
                    has_name = true;
                    name
                },
                None => {
                    has_name = false;
                    "".to_string()
                },
            };
            match path {
                Some(path) => doc = Doc::new("", &path, "", "", "", "", "", &name),
                None => doc = Doc::new("", ".", "", "", "", "", "", &name),
            };

            match doc.update_project(has_name) {
                Ok(_) => { },
                Err(e) => { eprintln!("update project failed ({})", e) },
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

            if install {
                match Repository::clone_recurse("https://github.com/wang-borong/omnidoc-libs", &olib) {
                    Ok(_) => println!("install {} success", olib.display()),
                    Err(e) => eprintln!("failed to clone {}", e),
                };

            } else if update {
                match git_pull(&olib, "origin", "main") {
                    Ok(_) => println!("update {} success", olib.display()),
                    Err(e) => eprintln!("update {} failed {}", olib.display(), e),
                }
            }
        }
        Commands::List => {
            println!(r#"document types:
  ebook-md  (elegantbook class based markdown document writing system)
  enote-md  (elegantnote class based markdown document writing system)
  ebook-tex (elegantbook class based latex document writing system)
  enote-tex (elegantnote class based latex document writing system)
  myart-tex (myart class based latex document writing system)
  mybook-tex (mybook class based latex document writing system)"#);
        }
    }
}

