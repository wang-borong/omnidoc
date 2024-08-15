use clap::{Command, CommandFactory, Parser, Subcommand, ValueHint};
use clap_complete::{generate, Generator, Shell};
use dirs::{data_local_dir, config_local_dir};
use std::env;

use omnidoc::doc::Doc;
use omnidoc::config::ConfigParser;
use omnidoc::git::{git_clone, git_pull};
use omnidoc::fs;

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
        /// select a document type
        #[arg(short = 'T', long)]
        doctype: String,

        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
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
        /// select a document type
        #[arg(short = 'T', long)]
        doctype: String,

        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: String,
    },

    /// build the document project
    Build {
        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,
        /// set the output path
        #[arg(short, long, value_hint = ValueHint::AnyPath)]
        output: Option<String>,

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

    /// generate shell completion
    Complete {
        /// If provided, outputs the completion file for given shell
        #[arg(long = "generate", value_enum)]
        generator: Option<Shell>,
    },
}

fn print_completions<G: Generator>(gen: G, cmd: &mut Command) {
    generate(gen, cmd, cmd.get_name().to_string(), &mut std::io::stdout());
}

pub fn cli() {
    let args = OmniCli::parse();

    match args.command {
        Commands::Init { path, author, version, release, language, title, doctype } => {
            let mut config_parser = ConfigParser::default();
            match config_parser.parse() {
                Ok(()) => { },
                Err(e) => eprintln!("Parse config failed ({})", e),
            }
            let envs = config_parser.get_envs().expect("Unable get envs");

            let author_conf = config_parser.get_author_name();
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
                &release, &language, &doctype, "");
            match doc.init_project(envs, false) {
                Ok(_) => { },
                Err(e) => { eprintln!("Initial project failed ({})", e) },
            }
        },
        Commands::Build { path, output, verbose } => {
            let mut config_parser = ConfigParser::default();
            match config_parser.parse() {
                Ok(()) => { },
                Err(e) => eprintln!("Parse config failed ({})", e),
            }
            let envs = config_parser.get_envs().expect("Unable get envs");

            let doc: Doc;
            match path {
                Some(path) => doc = Doc::new("", &path, "", "", "", "", "", ""),
                None => doc = Doc::new("", ".", "", "", "", "", "", ""),
            };
            match doc.build_project(output, envs, verbose) {
                Ok(_) => { },
                Err(e) => { eprintln!("Build project failed ({})", e) },
            }
        },
        Commands::Open { path } => {
            let mut config_parser = ConfigParser::default();
            match config_parser.parse() {
                Ok(()) => { },
                Err(e) => eprintln!("Parse config failed ({})", e),
            }
            let envs = config_parser.get_envs().expect("Unable get envs");

            let doc: Doc;
            match path {
                Some(path) => doc = Doc::new("", &path, "", "", "", "", "", ""),
                None => doc = Doc::new("", ".", "", "", "", "", "", ""),
            };
            match doc.open_doc(envs) {
                Ok(_) => { },
                Err(e) => { eprintln!("Open doc failed ({})", e) },
            }
        },
        Commands::Clean { path, distclean } => {
            let mut config_parser = ConfigParser::default();
            match config_parser.parse() {
                Ok(()) => { },
                Err(e) => eprintln!("Parse config failed ({})", e),
            }
            let envs = config_parser.get_envs().expect("Unable get envs");

            let doc: Doc;
            match path {
                Some(path) => doc = Doc::new("", &path, "", "", "", "", "", ""),
                None => doc = Doc::new("", ".", "", "", "", "", "", ""),
            };

            match doc.clean_project(envs, distclean) {
                Ok(_) => { },
                Err(e) => { eprintln!("Clean project failed ({})", e) },
            }
        },
        Commands::New { path, author, version, release, language, title, doctype } => {
            let mut config_parser = ConfigParser::default();
            match config_parser.parse() {
                Ok(()) => { },
                Err(e) => eprintln!("Parse config failed ({})", e),
            }
            let envs = config_parser.get_envs().expect("Unable get envs");

            let author_conf = config_parser.get_author_name();
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
                &release, &language, &doctype, "");
            match doc.create_project(envs) {
                Ok(_) => { },
                Err(e) => {
                    match env::set_current_dir("..") {
                        Ok(_) => { },
                        Err(e) => eprintln!("Change dir to .. failed ({})", e),
                    }
                    match fs::remove_dir_all(&path) {
                        Ok(_) => { },
                        Err(e) => eprintln!("Remove '{}' failed ({})", &path, e),
                    }
                    eprintln!("Create project failed ({})", e);
                },
            }
        }
        Commands::Update { path } => {
            let mut config_parser = ConfigParser::default();
            match config_parser.parse() {
                Ok(()) => { },
                Err(e) => eprintln!("Parse config failed ({})", e),
            }
            let envs = config_parser.get_envs().expect("Unable get envs");

            let mut doc: Doc;

            match path {
                Some(path) => doc = Doc::new("", &path, "", "", "", "", "", ""),
                None => doc = Doc::new("", ".", "", "", "", "", "", ""),
            };

            match doc.update_project(envs) {
                Ok(_) => { },
                Err(e) => { eprintln!("Update project failed ({})", e) },
            }
        }
        Commands::Config {authors, lib, outdir, texmfhome, bibinputs, texinputs, force} => {
            let config_parser = ConfigParser::default();

            match config_parser.gen(authors, lib, outdir, texmfhome, bibinputs, texinputs, force) {
                Ok(_) => println!("Generate configuration success"),
                Err(e)  => eprintln!("Generate configuration failed ({})", e),
            }
        }
        Commands::Lib { install, update } => {
            let dld = data_local_dir().unwrap();
            let olib = dld.join("omnidoc");

            if install {
                match git_clone("https://github.com/wang-borong/omnidoc-libs", &olib, true) {
                    Ok(_) => println!("Install '{}' success", olib.display()),
                    Err(e) => eprintln!("Clone omnidoc-libs failed ({})", e),
                };

            } else if update {
                match git_pull(&olib, "origin", "main") {
                    Ok(_) => println!("Update '{}' success", olib.display()),
                    Err(e) => eprintln!("Update {} failed ({})", olib.display(), e),
                }
            }

            let mut latexmkrc = config_local_dir().unwrap();

            latexmkrc.push("latexmk");
            if !latexmkrc.exists() {
                match fs::create_dir_all(&latexmkrc) {
                    Ok(_) => { },
                    Err(e) => eprintln!("Create latexmk config dir failed ({})", e),
                }
            }

            latexmkrc.push("latexmkrc");
            if !latexmkrc.exists() {
                match fs::copy_from_lib("repo/latexmkrc", &latexmkrc) {
                    Ok(_) => { },
                    Err(e) => eprintln!("Setup latexmkrc failed ({})", e),
                }
            }
        }
        Commands::List => {
            println!(r#"Document types:
  ebook-md  (elegantbook class based markdown document writing system)
  enote-md  (elegantnote class based markdown document writing system)
  ebook-tex (elegantbook class based latex document writing system)
  enote-tex (elegantnote class based latex document writing system)
  myart-tex (myart class based latex document writing system)
  myrep-tex (myrep class based latex document writing system)
  mybook-tex (mybook class based latex document writing system)
  resume-ng-tex"#);
        }
        Commands::Complete { generator } => {
            if let Some(generator) = generator {
                let mut cmd = OmniCli::command();

                print_completions(generator, &mut cmd);
            }
        }
    }
}

