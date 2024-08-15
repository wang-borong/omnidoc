use clap::{Command, CommandFactory, Parser, Subcommand, ValueHint};
use clap_complete::{generate, Generator, Shell};
use dirs::{data_local_dir, config_local_dir};
use std::env;
use std::path::Path;
use std::process::exit;

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

        /// set the document title
        #[arg(short = 't', long)]
        title: String,
        /// select a document type
        #[arg(short = 'T', long)]
        doctype: String,

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

    /// generate shell completion
    Complete {
        /// If provided, outputs the completion file for given shell
        #[arg(short, long = "generate", value_enum)]
        generator: Option<Shell>,
    },
}

fn print_completions<G: Generator>(gen: G, cmd: &mut Command) {
    generate(gen, cmd, cmd.get_name().to_string(), &mut std::io::stdout());
}

fn omnidoc_lib_exists() -> bool {
    let local_data_dir = data_local_dir().unwrap();
    let omnidoc_lib_dir = local_data_dir.join("omnidoc");

    omnidoc_lib_dir.exists()
}

pub fn cli() {
    let args = OmniCli::parse();

    match args.command {
        Commands::New { .. } | Commands::Init { .. }
        | Commands::Update { .. } | Commands::Build { .. }
        | Commands::Clean { .. } => {
            if !omnidoc_lib_exists() {
                eprintln!("No omnidoc lib installed, please install it by \
                    'omnidoc lib --install'");
                exit(1);
            }
        },
        _ => { },
    }

    match args.command {
        // NOTE: Get into the project directory
        Commands::New { ref path, .. } => {
            if !Path::new(&path).exists() {
                match fs::create_dir_all(&path) {
                    Ok(_) => { },
                    Err(e) => {
                        eprintln!("Cannot create directory '{}' ({})", path, e);
                        exit(1);
                    },
                }
                match env::set_current_dir(&path) {
                    Ok(_) => { },
                    Err(e) => {
                        eprintln!("Cannot change directory to '{}' ({})", path, e);
                        exit(1);
                    },
                }
            }
        },
        Commands::Init { ref path, .. } | Commands::Build { ref path, .. } 
        | Commands::Open { ref path } | Commands::Clean { ref path, .. }
        | Commands::Update { ref path } => {
            if let Some(path) = path {
                match env::set_current_dir(&path) {
                    Ok(_) => { },
                    Err(e) => {
                        eprintln!("Cannot change directory to {} ({})", path, e);
                        exit(1);
                    },
                }
            }
        },
        _ => { },
    }

    match args.command {
        Commands::New { path, author, title, doctype } => {
            if Path::new(&path).exists() {
                println!("The path exists already, no action");
                exit(1);
            }

            let config_parser = match ConfigParser::default() {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Get default config faild ({})", e);
                    exit(1);
                },
            };
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

            let doc = Doc::new(&title, &path, &author, &doctype, envs);
            match doc.create_project() {
                Ok(_) => { },
                Err(e) => {
                    match env::set_current_dir("..") {
                        Ok(_) => { },
                        Err(e) => {
                            eprintln!("Change dir to .. failed ({})", e);
                            exit(1);
                        },
                    }
                    match fs::remove_dir_all(&path) {
                        Ok(_) => { },
                        Err(e) => {
                            eprintln!("Remove '{}' failed ({})", &path, e);
                            exit(1);
                        },
                    }
                    eprintln!("Create project failed ({})", e);
                    exit(1);
                },
            }
        },
        Commands::Init { path, author, title, doctype } => {
            let path = match path {
                Some(path) => path,
                None => ".".to_owned(),
            };

            if !Path::new(&path).exists() {
                println!("The path doesn't exist, no action");
                exit(1);
            }

            let config_parser = match ConfigParser::default() {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Get default config faild ({})", e);
                    exit(1);
                },
            };
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

            let doc = Doc::new(&title, &path, &author, &doctype, envs);
            if Doc::is_omnidoc_project() {
                println!("It is an omnidoc project already, no action");
                return;
            }
            match doc.init_project(false) {
                Ok(_) => { },
                Err(e) => {
                    eprintln!("Initial project failed ({})", e);
                    exit(1);
                },
            }
        },
        Commands::Build { path, verbose } => {
            let config_parser = match ConfigParser::default() {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Get default config faild ({})", e);
                    exit(1);
                },
            };
            let envs = config_parser.get_envs().expect("Unable get envs");

            let path = match path {
                Some(path) => path,
                None => ".".to_owned(),
            };
            let doc = Doc::new("", &path, "", "", envs);
            match doc.build_project(verbose) {
                Ok(_) => { },
                Err(e) => {
                    eprintln!("Build project failed ({})", e);
                    exit(1);
                },
            }
        },
        Commands::Open { path } => {
            let config_parser = match ConfigParser::default() {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Get default config faild ({})", e);
                    exit(1);
                },
            };
            let envs = config_parser.get_envs().expect("Unable get envs");

            let path = match path {
                Some(path) => path,
                None => ".".to_owned(),
            };
            let doc = Doc::new("", &path, "", "", envs);
            match doc.open_doc() {
                Ok(_) => { },
                Err(e) => {
                    eprintln!("Open doc failed ({})", e);
                    exit(1);
                },
            }
        },
        Commands::Clean { path, distclean } => {
            let config_parser = match ConfigParser::default() {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Get default config faild ({})", e);
                    exit(1);
                },
            };
            let envs = config_parser.get_envs().expect("Unable get envs");

            let path = match path {
                Some(path) => path,
                None => ".".to_owned(),
            };
            let doc: Doc = Doc::new("", &path, "", "", envs);

            match doc.clean_project(distclean) {
                Ok(_) => { },
                Err(e) => {
                    eprintln!("Clean project failed ({})", e);
                    exit(1);
                },
            }
        },
        Commands::Update { path } => {
            let config_parser = match ConfigParser::default() {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Get default config faild ({})", e);
                    exit(1);
                },
            };
            let envs = config_parser.get_envs().expect("Unable get envs");

            let path = match path {
                Some(path) => path,
                None => ".".to_owned(),
            };

            let mut doc = Doc::new("", &path, "", "", envs);

            match doc.update_project() {
                Ok(_) => { },
                Err(e) => {
                    eprintln!("Update project failed ({})", e);
                    exit(1);
                },
            }
        }
        Commands::Config {authors, lib, outdir, texmfhome, bibinputs, texinputs, force} => {
            match ConfigParser::gen(authors, lib, outdir, texmfhome, bibinputs, texinputs, force) {
                Ok(_) => println!("Generate configuration success"),
                Err(e)  => {
                    eprintln!("Generate configuration failed ({})", e);
                    exit(1);
                },
            }
        },
        Commands::Lib { update, .. } => {
            let dld = data_local_dir().unwrap();
            let olib = dld.join("omnidoc");

            if update {
                match git_pull(&olib, "origin", "main") {
                    Ok(_) => println!("Update '{}' success", olib.display()),
                    Err(e) => {
                        eprintln!("Update {} failed ({})", olib.display(), e);
                        exit(1);
                    },
                }
            } else {
                match git_clone("https://github.com/wang-borong/omnidoc-libs", &olib, true) {
                    Ok(_) => println!("Install '{}' success", olib.display()),
                    Err(e) => {
                        eprintln!("Install omnidoc-libs failed ({})", e);
                        exit(1);
                    },
                };
            }

            let mut latexmkrc = config_local_dir().unwrap();

            latexmkrc.push("latexmk");
            if !latexmkrc.exists() {
                match fs::create_dir_all(&latexmkrc) {
                    Ok(_) => { },
                    Err(e) => {
                        eprintln!("Create latexmk config dir failed ({})", e);
                        exit(1);
                    },
                }
            }

            latexmkrc.push("latexmkrc");
            if !latexmkrc.exists() {
                match fs::copy_from_lib("repo/latexmkrc", &latexmkrc) {
                    Ok(_) => { },
                    Err(e) => {
                        eprintln!("Setup latexmkrc failed ({})", e);
                        exit(1);
                    },
                }
            }
        },
        Commands::List => {
            println!(r#"Current supported document types:
✅ ebook-md  (elegantbook class based markdown document writing system)
✅ enote-md  (elegantnote class based markdown document writing system)
✅ ebook-tex (elegantbook class based latex document writing system)
✅ enote-tex (elegantnote class based latex document writing system)
✅ myart-tex (myart class based latex document writing system)
✅ myrep-tex (myrep class based latex document writing system)
✅ mybook-tex (mybook class based latex document writing system)
✅ resume-ng-tex"#);
        },
        Commands::Complete { generator } => {
            if let Some(generator) = generator {
                let mut cmd = OmniCli::command();

                print_completions(generator, &mut cmd);
            }
        },
    }
}

