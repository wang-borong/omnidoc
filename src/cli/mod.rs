pub mod commands;
pub mod handlers;
pub mod utils;

use crate::error::{OmniDocError, Result};
use clap::Parser;
use clap::{Command, CommandFactory};
use clap_complete::{generate, Generator};
use commands::{
    CheckSubcommand, Commands, ConfigSubcommand, ConvertSubcommand, LibSubcommand, OmniCli,
    PluginSubcommand, TemplateSubcommand,
};
use handlers::*;
use std::env;
use std::path::Path;
use utils::*;

fn print_completions<G: Generator>(gen: G, cmd: &mut Command) {
    generate(gen, cmd, cmd.get_name().to_string(), &mut std::io::stdout());
}

fn command_needs_library(command: &Commands) -> bool {
    matches!(
        command,
        Commands::Build { .. }
            | Commands::Watch { .. }
            | Commands::Publish { verify: false, .. }
            | Commands::Ci { .. }
            | Commands::Md2pdf { .. }
            | Commands::Md2html { .. }
            | Commands::Convert { .. }
            | Commands::Check {
                subcommand: CheckSubcommand::Ci { .. }
            }
    )
}

fn prepare_working_directory(command: &mut Commands) -> Result<()> {
    let path = match command {
        Commands::Init { path, .. } => path,
        _ => return Ok(()),
    };

    let Some(path) = path else {
        return Ok(());
    };
    let canonical = Path::new(path).canonicalize().map_err(|_| {
        OmniDocError::Project(format!(
            "Path does not exist or is not accessible: {}",
            path
        ))
    })?;
    if !canonical.is_dir() {
        return Err(OmniDocError::Project(format!(
            "Path is not a directory: {}",
            canonical.display()
        )));
    }
    env::set_current_dir(&canonical).map_err(OmniDocError::Io)?;
    *path = canonical.to_string_lossy().to_string();
    Ok(())
}

/// Main CLI entry point
pub fn cli() -> Result<()> {
    let mut args = OmniCli::parse();

    let orig_path = env::current_dir().map_err(OmniDocError::Io)?;

    // Ensure the release-bound library exists for commands that need it.
    if command_needs_library(&args.command) && !omnidoc_lib_exists() {
        handle_lib(true, false, false, false, false)?;
    }

    prepare_working_directory(&mut args.command)?;

    // Route to appropriate command handler
    match args.command {
        Commands::New {
            path,
            author,
            title,
            doctype,
            format,
            defaults,
        } => {
            handle_new(&orig_path, path, title, author, doctype, format, defaults)?;
        }
        Commands::Init {
            author,
            title,
            doctype,
            format,
            defaults,
            no_commit,
            ..
        } => {
            handle_init(title, author, doctype, format, defaults, no_commit)?;
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
        Commands::Check { subcommand } => {
            handle_check(subcommand)?;
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
            subcommand,
            path,
            json,
            validate,
        } => match subcommand {
            Some(PluginSubcommand::List { path, json }) => handle_plugin(path, json, false)?,
            Some(PluginSubcommand::Validate { path, json }) => handle_plugin(path, json, true)?,
            None => handle_plugin(path, json, validate)?,
        },
        Commands::Status { path, json } => {
            handle_status(path, json)?;
        }
        Commands::Open {
            path,
            to,
            print_path,
        } => {
            handle_open(path, to, print_path)?;
        }
        Commands::Clean {
            path,
            distclean,
            dry_run,
            json,
        } => {
            handle_clean(path, distclean, dry_run, json)?;
        }
        Commands::Update {
            path,
            dry_run,
            diff,
            no_commit,
            json,
        } => {
            handle_update(path, dry_run, diff, no_commit, json)?;
        }
        Commands::Config {
            subcommand,
            authors,
            lib,
            outdir,
            texmfhome,
            bibinputs,
            texinputs,
            force,
        } => match subcommand {
            Some(ConfigSubcommand::Init {
                author,
                lib,
                outdir,
                texmfhome,
                bibinputs,
                texinputs,
                force,
            }) => handle_config(author, lib, outdir, texmfhome, bibinputs, texinputs, force)?,
            Some(ConfigSubcommand::Show { path, scope, json }) => {
                handle_config_show(path, scope, json)?;
            }
            Some(ConfigSubcommand::Get {
                key,
                path,
                scope,
                json,
            }) => {
                handle_config_get(key, path, scope, json)?;
            }
            None => {
                let author = authors.ok_or_else(|| {
                        OmniDocError::Config(
                            "An author is required. Use `omnidoc config init --author NAME`, or inspect configuration with `omnidoc config show`."
                                .to_string(),
                        )
                    })?;
                handle_config(author, lib, outdir, texmfhome, bibinputs, texinputs, force)?;
            }
        },
        Commands::Lib {
            subcommand,
            install,
            update,
            status,
            verify,
            json,
        } => match subcommand {
            Some(LibSubcommand::Install { json }) => handle_lib(true, false, false, false, json)?,
            Some(LibSubcommand::Update { json }) => handle_lib(false, true, false, false, json)?,
            Some(LibSubcommand::Status { json }) => handle_lib(false, false, true, false, json)?,
            Some(LibSubcommand::Verify { json }) => handle_lib(false, false, false, true, json)?,
            None => handle_lib(install, update, status, verify, json)?,
        },
        Commands::Theme { subcommand } => {
            handle_theme(subcommand)?;
        }
        Commands::List => {
            print_doctypes();
        }
        Commands::Complete { shell, generator } => {
            let generator = shell.or(generator).ok_or_else(|| {
                OmniDocError::Other(
                    "A shell is required. Example: `omnidoc complete zsh`".to_string(),
                )
            })?;
            let mut cmd = OmniCli::command();
            print_completions(generator, &mut cmd);
        }
        Commands::Template {
            subcommand,
            validate,
        } => {
            if validate {
                handle_template_validate(None, false)?;
            } else {
                match subcommand {
                    Some(TemplateSubcommand::List { format, json }) => {
                        print_templates(format, json)?;
                    }
                    Some(TemplateSubcommand::Validate { key, json }) => {
                        handle_template_validate(key, json)?;
                    }
                    None => print_templates(None, false)?,
                }
            }
        }
        Commands::Convert { subcommand } => {
            handle_convert(subcommand)?;
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

fn handle_check(subcommand: CheckSubcommand) -> Result<()> {
    match subcommand {
        CheckSubcommand::Doctor {
            path,
            json,
            strict,
            outputs,
        } => handle_doctor(path, json, strict, outputs),
        CheckSubcommand::Config { path } => handle_config_validate(path),
        CheckSubcommand::Lint { path, strict } => handle_lint(path, strict),
        CheckSubcommand::Deps { path, json } => handle_deps(path, json),
        CheckSubcommand::Lock {
            path,
            check,
            update,
        } => handle_lock(path, check, update),
        CheckSubcommand::Ci { path, outputs } => handle_ci(path, outputs),
    }
}

fn handle_convert(subcommand: ConvertSubcommand) -> Result<()> {
    match subcommand {
        ConvertSubcommand::Pdf {
            lang,
            inputs,
            output,
        } => handle_md2pdf(lang, inputs, output),
        ConvertSubcommand::Html {
            inputs,
            output,
            css,
        } => handle_md2html(inputs, output, css),
    }
}
