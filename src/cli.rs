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

        /// give the doc a title
        #[arg(long)]
        title: String,
        /// give the output doc a name
        #[arg(long)]
        docname: String,
        /// select a document type to create
        #[arg(long)]
        doctype: String,

        /// set project path to create the documentation project
        path: String,

        // create makefile
        //#[arg(long, default_value_t = false)]
        //makefile: bool,
    },

    /// init a project
    Init {
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

        /// give the doc a title
        #[arg(long)]
        title: String,
        /// give the output doc a name
        #[arg(long)]
        docname: String,
        /// select a document type to create
        #[arg(long)]
        doctype: String,

        /// path to documentation project
        path: String,
    },

    /// build the document project
    Build {
        /// builder to use (default to 'pdf')
        #[arg(short, long)]
        builder: Option<String>,

        /// path to documentation project
        path: Option<String>,
        /// path to output directory
        output: Option<String>,

    },

    /// clean the document project
    Clean {
        /// distclean project
        #[arg(short, long)]
        distclean: bool,

        /// path to documentation project
        path: Option<String>,

    },

    /// update doc repo
    Update {
        /// path to documentation project
        path: Option<String>,
    },

    /// generate configuration
    Config,

    /// omnidoc library maintenance
    Lib {
        /// install omnidoc lib to XDG_DATA_DIR
        #[arg(short, long)]
        install: bool,

        /// update omnidoc lib
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
        Commands::Init { path, author, docver, release, language, title, docname, doctype } => {
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
            let doc = Doc::new(&title, &path, &author, &docver,
                &release, &language, &doctype, &docname);
            match doc.init_project() {
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
        Commands::Create { path, author, docver, release, language, title, docname, doctype } => {
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
            let doc = Doc::new(&title, &path, &author, &docver,
                &release, &language, &doctype, &docname);
            match doc.create_project() {
                Ok(_) => { },
                Err(e) => { eprintln!("create project failed ({})", e) },
            }
        }
        Commands::Update { path } => {
            let doc: Doc;
            match path {
                Some(path) => doc = Doc::new("", &path, "", "", "", "", "", ""),
                None => doc = Doc::new("", ".", "", "", "", "", "", ""),
            };

            match doc.update_project() {
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
            println!(r#"doctypes:
  ebook-md  (elegantbook class based markdown document writing system)
  enote-md  (elegantnote class based markdown document writing system)
  ebook-tex (elegantbook class based latex document writing system)
  enote-tex (elegantnote class based latex document writing system)
  myart-tex (myart class based latex document writing system)
  mybook-tex (mybook class based latex document writing system)"#);
        }
    }
}

