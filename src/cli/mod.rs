pub mod commands;
pub mod handlers;
pub mod utils;

use crate::constants::git_refs;
use crate::error::{OmniDocError, Result};
use crate::git::{git_clone, git_pull};
use clap::Parser;
use clap::{Command, CommandFactory};
use clap_complete::{generate, Generator};
use commands::{Commands, OmniCli};
use dirs::data_local_dir;
use handlers::*;
use std::env;
use std::path::Path;
use utils::*;

fn print_completions<G: Generator>(gen: G, cmd: &mut Command) {
    generate(gen, cmd, cmd.get_name().to_string(), &mut std::io::stdout());
}

/// Main CLI entry point
pub fn cli() -> Result<()> {
    let args = OmniCli::parse();

    let orig_path = env::current_dir().map_err(|e| OmniDocError::Io(e))?;

    // Ensure omnidoc lib exists for commands that need it
    match args.command {
        Commands::New { .. } | Commands::Init { .. } | Commands::Build { .. } => {
            if !omnidoc_lib_exists() {
                let dld = data_local_dir()
                    .ok_or_else(|| OmniDocError::Other("data_local_dir not found".to_string()))?;
                let olib = dld.join("omnidoc");
                let _ = git_clone("https://github.com/wang-borong/omnidoc-libs", &olib, true);
            } else {
                let dld = data_local_dir()
                    .ok_or_else(|| OmniDocError::Other("data_local_dir not found".to_string()))?;
                let olib = dld.join("omnidoc");
                let _ = git_pull(&olib, git_refs::ORIGIN, git_refs::MAIN_BRANCH);
            }
        }
        _ => {}
    }

    // Handle directory changes for commands that need it
    match args.command {
        Commands::Init { ref path, .. }
        | Commands::Build { ref path, .. }
        | Commands::Open { ref path }
        | Commands::Clean { ref path, .. }
        | Commands::Update { ref path } => {
            if let Some(path) = path {
                if !Path::new(&path).exists() {
                    return Err(OmniDocError::Project(format!(
                        "The path doesn't exist: {}",
                        path
                    )));
                }
                env::set_current_dir(&path).map_err(|e| OmniDocError::Io(e))?;
            }
        }
        _ => {}
    }

    // Route to appropriate command handler
    match args.command {
        Commands::New {
            path,
            author,
            title,
        } => {
            handle_new(&orig_path, path, title, author)?;
        }
        Commands::Init {
            path,
            author,
            title,
        } => {
            handle_init(&orig_path, path, title, author)?;
        }
        Commands::Build { path, verbose } => {
            handle_build(path, verbose)?;
        }
        Commands::Open { path } => {
            handle_open(path)?;
        }
        Commands::Clean { path, distclean } => {
            handle_clean(path, distclean)?;
        }
        Commands::Update { path } => {
            handle_update(path)?;
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
            handle_config(authors, lib, outdir, texmfhome, bibinputs, texinputs, force)?;
        }
        Commands::Lib { update, .. } => {
            handle_lib(update)?;
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

    Ok(())
}
