use crate::cli::handlers::build::{build_project_outputs, BuildRunOptions};
use crate::cli::handlers::common::{create_config_manager, create_config_manager_default};
use crate::config::CliOverrides;
use crate::error::{OmniDocError, Result};
use crate::project_tools;
use crate::utils::path;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize)]
struct DoctorCheck {
    name: String,
    ok: bool,
    detail: String,
}

pub fn handle_doctor(path: Option<String>, json: bool) -> Result<()> {
    let project_path = path::determine_project_path(path)?.canonicalize()?;
    let config_manager = create_config_manager_default(Some(&project_path))?;
    let config = config_manager.get_merged().clone();
    let mut checks = Vec::new();

    for tool in ["pandoc", "pandoc-crossref", "latexmk", "xelatex"] {
        let found = which::which(tool).ok();
        checks.push(DoctorCheck {
            name: tool.to_string(),
            ok: found.is_some(),
            detail: found
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "not found".to_string()),
        });
    }

    if let Some(lib_path) = &config.lib_path {
        checks.push(DoctorCheck {
            name: "omnidoc-libs".to_string(),
            ok: Path::new(lib_path).exists(),
            detail: lib_path.clone(),
        });
    }

    let issues = project_tools::validate_config(&project_path, &config);
    checks.push(DoctorCheck {
        name: "config".to_string(),
        ok: !project_tools::has_errors(&issues),
        detail: format!("{} issue(s)", issues.len()),
    });

    if json {
        let content = serde_json::to_string_pretty(&checks)
            .map_err(|err| OmniDocError::Other(err.to_string()))?;
        println!("{}", content);
    } else {
        for check in checks {
            println!(
                "{} {} - {}",
                if check.ok { "ok" } else { "fail" },
                check.name,
                check.detail
            );
        }
        if !issues.is_empty() {
            project_tools::print_issues(&issues);
        }
    }

    Ok(())
}

pub fn handle_config_validate(path: Option<String>) -> Result<()> {
    let project_path = path::determine_project_path(path)?.canonicalize()?;
    let config_manager = create_config_manager_default(Some(&project_path))?;
    let issues = project_tools::validate_config(&project_path, config_manager.get_merged());
    project_tools::print_issues(&issues);
    if project_tools::has_errors(&issues) {
        return Err(OmniDocError::Config(
            "configuration validation failed".to_string(),
        ));
    }
    Ok(())
}

pub fn handle_lint(path: Option<String>, strict: bool) -> Result<()> {
    let project_path = path::determine_project_path(path)?.canonicalize()?;
    let config_manager = create_config_manager_default(Some(&project_path))?;
    let mut issues = project_tools::validate_config(&project_path, config_manager.get_merged());
    issues.extend(project_tools::lint_project(&project_path));
    issues.extend(project_tools::run_plugin_lint_rules(
        &project_path,
        config_manager.get_merged(),
    ));
    project_tools::print_issues(&issues);
    if (strict && project_tools::has_warnings_or_errors(&issues))
        || project_tools::has_errors(&issues)
    {
        return Err(OmniDocError::Project("lint failed".to_string()));
    }
    Ok(())
}

pub fn handle_deps(path: Option<String>, json: bool) -> Result<()> {
    let project_path = path::determine_project_path(path)?.canonicalize()?;
    let config_manager = create_config_manager_default(Some(&project_path))?;
    let graph = project_tools::dependency_graph(&project_path, config_manager.get_merged());

    if json {
        let content = serde_json::to_string_pretty(&graph)
            .map_err(|err| OmniDocError::Other(err.to_string()))?;
        println!("{}", content);
    } else {
        for file in graph.files {
            println!("{}", file);
        }
    }

    Ok(())
}

pub fn handle_ci(path: Option<String>, outputs: Vec<String>) -> Result<()> {
    let project_path = path::determine_project_path(path)?.canonicalize()?;
    let cli_overrides = CliOverrides::new()
        .with_outputs(outputs)
        .with_latex_backend(Some("latexmk".to_string()));
    build_project_outputs(
        &project_path,
        cli_overrides,
        true,
        BuildRunOptions {
            force: true,
            report: true,
            write_lock: true,
            strict: true,
        },
        true,
    )
}

pub fn handle_lock(path: Option<String>, check: bool, update: bool) -> Result<()> {
    let project_path = path::determine_project_path(path)?.canonicalize()?;
    let config_manager = create_config_manager_default(Some(&project_path))?;
    let graph = project_tools::dependency_graph(&project_path, config_manager.get_merged());
    if check {
        let status = project_tools::check_lock(&project_path, config_manager.get_merged(), &graph)?;
        if status.up_to_date {
            println!("omnidoc.lock is up to date");
            return Ok(());
        }
        let content = serde_json::to_string_pretty(&status)
            .map_err(|err| OmniDocError::Other(err.to_string()))?;
        println!("{}", content);
        return Err(OmniDocError::Project(
            "omnidoc.lock is missing or out of date".to_string(),
        ));
    }
    if !update && project_path.join("omnidoc.lock").exists() {
        println!("omnidoc.lock already exists; use --update to rewrite it");
        return Ok(());
    }
    project_tools::write_lock(&project_path, config_manager.get_merged(), &graph)?;
    println!("Wrote {}", project_path.join("omnidoc.lock").display());
    Ok(())
}

pub fn handle_plugin(path: Option<String>, json: bool, validate: bool) -> Result<()> {
    let project_path = path::determine_project_path(path)?.canonicalize()?;
    let config_manager = create_config_manager(Some(&project_path), CliOverrides::new())?;
    let plugins = project_tools::discovered_plugins(&project_path, config_manager.get_merged());
    if json {
        let content = serde_json::to_string_pretty(&plugins)
            .map_err(|err| OmniDocError::Other(err.to_string()))?;
        println!("{}", content);
    } else if plugins.is_empty() {
        println!("No plugins or external templates discovered.");
    } else {
        for plugin in &plugins {
            let status = if plugin.valid { "ok" } else { "fail" };
            if let Some(error) = &plugin.error {
                println!("{} {} ({}) - {}", status, plugin.key, plugin.path, error);
            } else {
                println!("{} {} ({})", status, plugin.key, plugin.path);
            }
        }
    }

    if validate && plugins.iter().any(|plugin| !plugin.valid) {
        return Err(OmniDocError::Project(
            "plugin validation failed".to_string(),
        ));
    }
    Ok(())
}
