pub mod commands;
pub mod handlers;
pub mod utils;

use crate::error::{OmniDocError, Result};
use clap::Parser;
use clap::{Command, CommandFactory};
use clap_complete::{generate, Generator};
use commands::{Commands, OmniCli};
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

    let orig_path = env::current_dir().map_err(OmniDocError::Io)?;

    // Ensure the release-bound library exists for commands that need it.
    match args.command {
        Commands::New { .. }
        | Commands::Init { .. }
        | Commands::Build { .. }
        | Commands::Watch { .. }
        | Commands::Publish { verify: false, .. }
        | Commands::Ci { .. }
        | Commands::Md2pdf { .. }
        | Commands::Md2html { .. }
            if !omnidoc_lib_exists() =>
        {
            handle_lib(true, false, false, false, false)?;
        }
        _ => {}
    }

    // Handle directory changes for commands that need it
    match args.command {
        Commands::Init { ref path, .. }
        | Commands::Open { ref path }
        | Commands::Clean { ref path, .. }
        | Commands::Update { ref path } => {
            if let Some(path) = path {
                if !Path::new(&path).exists() {
                    return Err(OmniDocError::Project(format!(
                        "Path does not exist: {}",
                        path
                    )));
                }
                env::set_current_dir(path).map_err(OmniDocError::Io)?;
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
        Commands::Build {
            path,
            to,
            all,
            outputs,
            pdf_engine,
            latex_backend,
            max_latex_passes,
            force,
            report,
            write_lock,
            strict,
            verbose,
        } => {
            handle_build(
                path,
                to,
                all,
                outputs,
                pdf_engine,
                latex_backend,
                max_latex_passes,
                force,
                report,
                write_lock,
                strict,
                verbose,
            )?;
        }
        Commands::Watch {
            path,
            to,
            all,
            outputs,
            pdf_engine,
            latex_backend,
            max_latex_passes,
            debounce_ms,
            once,
            force,
            report,
            strict,
            verbose,
        } => {
            handle_watch(
                path,
                to,
                all,
                outputs,
                pdf_engine,
                latex_backend,
                max_latex_passes,
                debounce_ms,
                once,
                force,
                report,
                strict,
                verbose,
            )?;
        }
        Commands::Publish {
            path,
            to,
            all,
            outputs,
            pdf_engine,
            latex_backend,
            max_latex_passes,
            dist_dir,
            tag,
            no_build,
            verify,
            json,
            force,
            strict,
            verbose,
        } => {
            handle_publish(
                path,
                to,
                all,
                outputs,
                pdf_engine,
                latex_backend,
                max_latex_passes,
                dist_dir,
                tag,
                no_build,
                verify,
                json,
                force,
                strict,
                verbose,
            )?;
        }
        Commands::Doctor {
            path,
            json,
            strict,
            outputs,
        } => {
            handle_doctor(path, json, strict, outputs)?;
        }
        Commands::ConfigValidate { path } => {
            handle_config_validate(path)?;
        }
        Commands::Lint { path, strict } => {
            handle_lint(path, strict)?;
        }
        Commands::Deps { path, json } => {
            handle_deps(path, json)?;
        }
        Commands::Ci { path, outputs } => {
            handle_ci(path, outputs)?;
        }
        Commands::Lock {
            path,
            check,
            update,
        } => {
            handle_lock(path, check, update)?;
        }
        Commands::Plugin {
            path,
            json,
            validate,
        } => {
            handle_plugin(path, json, validate)?;
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
        Commands::Lib {
            install,
            update,
            status,
            verify,
            json,
        } => {
            handle_lib(install, update, status, verify, json)?;
        }
        Commands::Theme { subcommand } => {
            handle_theme(subcommand)?;
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
        Commands::Template { validate } => {
            if validate {
                handle_template_validate();
            }
        }
        Commands::Md2pdf {
            lang,
            inputs,
            output,
        } => {
            handle_md2pdf(lang, inputs, output)?;
        }
        Commands::Md2html {
            inputs,
            output,
            css,
        } => {
            handle_md2html(inputs, output, css)?;
        }
        Commands::Fmt {
            paths,
            backup,
            check,
            diff,
            semantic,
            symbol,
        } => {
            handle_fmt(paths, backup, check, diff, semantic, symbol)?;
        }
        Commands::Figure {
            subcommand,
            format,
            force,
            output,
            sources,
        } => {
            handle_figure(subcommand, format, force, output, sources)?;
        }
    }

    Ok(())
}
