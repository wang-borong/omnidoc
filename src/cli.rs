use clap::{Command, CommandFactory, Parser, Subcommand, ValueHint};
use clap_complete::{generate, Generator, Shell};
use dirs::{config_local_dir, data_local_dir};
use std::env;
use std::path::Path;
use std::process::exit;

use omnidoc::config::ConfigParser;
use omnidoc::doc::Doc;
use omnidoc::fs;
use omnidoc::git::{git_clone, git_pull};
use omnidoc::rl::DTRL;

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

macro_rules! exit_eprintln {
    ($exit_code:expr, $($arg:tt)*) => {
        {
            eprintln!($($arg)*);
            exit($exit_code);
        }
    };
}

macro_rules! clean_exit_eprintln {
    ($exit_code:expr, $clean_block:block, $($arg:tt)*) => {
        {
            eprintln!($($arg)*);
            $clean_block
            exit($exit_code);
        }
    };
}

fn print_completions<G: Generator>(gen: G, cmd: &mut Command) {
    generate(gen, cmd, cmd.get_name().to_string(), &mut std::io::stdout());
}

fn omnidoc_lib_exists() -> bool {
    let local_data_dir = data_local_dir().unwrap();
    let omnidoc_lib_dir = local_data_dir.join("omnidoc");

    omnidoc_lib_dir.exists()
}

fn print_doctypes() {
    println!(
        r#"Current supported document types:
✅ ebook-md  (elegantbook class based markdown document writing system)
✅ enote-md  (elegantnote class based markdown document writing system)
✅ ctexart-tex (raw ctexart document type)
✅ ctexrep-tex (raw ctexrep document type)
✅ ctexbook-tex (raw ctexbook document type)
✅ ebook-tex (elegantbook class based latex document writing system)
✅ enote-tex (elegantnote class based latex document writing system)
✅ ctart-tex (ctart class based latex document writing system)
✅ ctrep-tex (ctrep class based latex document writing system)
✅ ctbook-tex (ctbook class based latex document writing system)
✅ resume-ng-tex
✅ moderncv-tex"#
    );
}

fn get_doctype_from_readline<O, N>(orig_path: O, path: N) -> String
where
    O: AsRef<Path>,
    N: AsRef<Path>,
{
    let mut dtrl = match DTRL::new() {
        Ok(dtrl) => dtrl,
        Err(e) => clean_exit_eprintln!(
            1,
            {
                let _ = env::set_current_dir(&orig_path);
                let _ = fs::remove_dir_all(&path);
            },
            "Create DTRL failed ({})",
            e
        ),
    };
    let mut doctype: String;
    loop {
        let readline = dtrl.readline();
        doctype = match readline {
            Ok(line) => line,
            Err(e) => {
                clean_exit_eprintln!(
                    1,
                    {
                        let _ = env::set_current_dir(&orig_path);
                        let _ = fs::remove_dir_all(&path);
                    },
                    "Get the input line failed ({})",
                    e
                );
            }
        };

        if &doctype == "list" || &doctype == "ls" {
            print_doctypes();
        } else {
            break;
        }
    }

    doctype
}

pub fn cli() {
    let args = OmniCli::parse();

    let orig_path = env::current_dir().unwrap();

    if !omnidoc_lib_exists() {
        let dld = data_local_dir().unwrap();
        let olib = dld.join("omnidoc");
        match git_clone("https://github.com/wang-borong/omnidoc-libs", &olib, true) {
            Ok(_) => { },
            Err(e) => {
                exit_eprintln!(1, "Install omnidoc-libs failed ({})", e);
            }
        };
    }

    match args.command {
        // NOTE: Enter into the project directory
        Commands::New { ref path, .. } => {
            if Path::new(&path).exists() {
                exit_eprintln!(1, "The path already exists, no action");
            }

            match fs::create_dir_all(&path) {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("Cannot create directory '{}' ({})", path, e);
                    exit(1);
                }
            }
            match env::set_current_dir(&path) {
                Ok(_) => {}
                Err(e) => {
                    clean_exit_eprintln!(
                        1,
                        {
                            let _ = env::set_current_dir(&orig_path);
                            let _ = fs::remove_dir_all(&path);
                        },
                        "Cannot change directory to '{}' ({})",
                        path,
                        e
                    );
                }
            }
        }
        Commands::Init { ref path, .. }
        | Commands::Build { ref path, .. }
        | Commands::Open { ref path }
        | Commands::Clean { ref path, .. }
        | Commands::Update { ref path } => {
            if let Some(path) = path {
                if !Path::new(&path).exists() {
                    exit_eprintln!(1, "The path doesn't exist, no action");
                }
                match env::set_current_dir(&path) {
                    Ok(_) => {}
                    Err(e) => {
                        exit_eprintln!(1, "Cannot change directory to {} ({})", path, e);
                    }
                }
            }
        }
        _ => {}
    }

    match args.command {
        Commands::New {
            path,
            author,
            title,
        } => {
            let config_parser = match ConfigParser::default() {
                Ok(c) => c,
                Err(e) => {
                    clean_exit_eprintln!(
                        1,
                        {
                            let _ = env::set_current_dir(&orig_path);
                            let _ = fs::remove_dir_all(&path);
                        },
                        "Get default config faild ({})",
                        e
                    );
                }
            };
            let envs = config_parser.get_envs().expect("Unable get envs");

            let author_conf = config_parser.get_author_name();
            let author = match author {
                Some(author) => author,
                None => match author_conf {
                    Ok(author) => author,
                    Err(_) => "Someone".to_string(),
                },
            };

            let doctype = get_doctype_from_readline(&orig_path, &path);

            let doc = Doc::new(&title, &path, &author, &doctype, envs);
            match doc.create_project() {
                Ok(_) => {}
                Err(e) => {
                    match env::set_current_dir("..") {
                        Ok(_) => {}
                        Err(e) => {
                            exit_eprintln!(1, "Change dir to .. failed ({})", e);
                        }
                    }
                    match fs::remove_dir_all(&path) {
                        Ok(_) => {}
                        Err(e) => {
                            exit_eprintln!(1, "Remove '{}' failed ({})", &path, e);
                        }
                    }
                    exit_eprintln!(1, "Create project failed ({})", e);
                }
            }
        }
        Commands::Init {
            path,
            author,
            title,
        } => {
            let path = match path {
                Some(path) => path,
                None => ".".to_owned(),
            };

            let config_parser = match ConfigParser::default() {
                Ok(c) => c,
                Err(e) => {
                    exit_eprintln!(1, "Get default config faild ({})", e);
                }
            };
            let envs = config_parser.get_envs().expect("Unable get envs");

            let author_conf = config_parser.get_author_name();
            let author = match author {
                Some(author) => author,
                None => match author_conf {
                    Ok(author) => author,
                    Err(_) => "Someone".to_string(),
                },
            };

            let doctype = get_doctype_from_readline(&orig_path, &path);

            let doc = Doc::new(&title, &path, &author, &doctype, envs);
            if Doc::is_omnidoc_project() {
                exit_eprintln!(1, "It is an omnidoc project already, no action");
            }
            match doc.init_project(false) {
                Ok(_) => {}
                Err(e) => {
                    exit_eprintln!(1, "Initial project failed ({})", e);
                }
            }
        }
        Commands::Build { path, verbose } => {
            let config_parser = match ConfigParser::default() {
                Ok(c) => c,
                Err(e) => {
                    exit_eprintln!(1, "Get default config faild ({})", e);
                }
            };
            let envs = config_parser.get_envs().expect("Unable get envs");

            let path = match path {
                Some(path) => path,
                None => ".".to_owned(),
            };
            let doc = Doc::new("", &path, "", "", envs);
            match doc.build_project(verbose) {
                Ok(_) => {}
                Err(e) => {
                    exit_eprintln!(1, "Build project failed ({})", e);
                }
            }
        }
        Commands::Open { path } => {
            let config_parser = match ConfigParser::default() {
                Ok(c) => c,
                Err(e) => {
                    exit_eprintln!(1, "Get default config faild ({})", e);
                }
            };
            let envs = config_parser.get_envs().expect("Unable get envs");

            let path = match path {
                Some(path) => path,
                None => ".".to_owned(),
            };
            let doc = Doc::new("", &path, "", "", envs);
            match doc.open_doc() {
                Ok(_) => {}
                Err(e) => {
                    exit_eprintln!(1, "Open doc failed ({})", e);
                }
            }
        }
        Commands::Clean { path, distclean } => {
            let config_parser = match ConfigParser::default() {
                Ok(c) => c,
                Err(e) => {
                    exit_eprintln!(1, "Get default config faild ({})", e);
                }
            };
            let envs = config_parser.get_envs().expect("Unable get envs");

            let path = match path {
                Some(path) => path,
                None => ".".to_owned(),
            };
            let doc: Doc = Doc::new("", &path, "", "", envs);

            match doc.clean_project(distclean) {
                Ok(_) => {}
                Err(e) => {
                    exit_eprintln!(1, "Clean project failed ({})", e);
                }
            }
        }
        Commands::Update { path } => {
            let config_parser = match ConfigParser::default() {
                Ok(c) => c,
                Err(e) => {
                    exit_eprintln!(1, "Get default config faild ({})", e);
                }
            };
            let envs = config_parser.get_envs().expect("Unable get envs");

            let path = match path {
                Some(path) => path,
                None => ".".to_owned(),
            };

            let mut doc = Doc::new("", &path, "", "", envs);

            match doc.update_project() {
                Ok(_) => {}
                Err(e) => {
                    exit_eprintln!(1, "Update project failed ({})", e);
                }
            }
        }
        Commands::Config {
            authors,
            lib,
            outdir,
            texmfhome,
            bibinputs,
            texinputs,
            force,
        } => {
            match ConfigParser::gen(authors, lib, outdir, texmfhome, bibinputs, texinputs, force) {
                Ok(_) => println!("Generate configuration success"),
                Err(e) => {
                    exit_eprintln!(1, "Generate configuration failed ({})", e);
                }
            }
        }
        Commands::Lib { update, .. } => {
            let dld = data_local_dir().unwrap();
            let olib = dld.join("omnidoc");

            if update {
                match git_pull(&olib, "origin", "main") {
                    Ok(_) => println!("Update '{}' success", olib.display()),
                    Err(e) => {
                        exit_eprintln!(1, "Update {} failed ({})", olib.display(), e);
                    }
                }
            } else {
                match git_clone("https://github.com/wang-borong/omnidoc-libs", &olib, true) {
                    Ok(_) => println!("Install '{}' success", olib.display()),
                    Err(e) => {
                        exit_eprintln!(1, "Install omnidoc-libs failed ({})", e);
                    }
                };
            }

            let mut latexmkrc = config_local_dir().unwrap();

            latexmkrc.push("latexmk");
            if !latexmkrc.exists() {
                match fs::create_dir_all(&latexmkrc) {
                    Ok(_) => {}
                    Err(e) => {
                        exit_eprintln!(1, "Create latexmk config dir failed ({})", e);
                    }
                }
            }

            latexmkrc.push("latexmkrc");
            if !latexmkrc.exists() {
                match fs::copy_from_lib("repo/latexmkrc", &latexmkrc) {
                    Ok(_) => {}
                    Err(e) => {
                        exit_eprintln!(1, "Setup latexmkrc failed ({})", e);
                    }
                }
            }
        }
        Commands::List => {
            print_doctypes();
        }
        Commands::Complete { generator } => {
            if let Some(generator) = generator {
                let mut cmd = OmniCli::command();

                print_completions(generator, &mut cmd);
            }
        }
    }
}
