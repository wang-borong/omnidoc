use crate::build::executor::BuildExecutor;
use crate::cli::handlers::build::{build_project_outputs, BuildRunOptions};
use crate::cli::handlers::common::{create_config_manager, create_config_manager_default};
use crate::cli::handlers::lib::library_diagnostic;
use crate::cli::handlers::theme::theme_diagnostic;
use crate::config::CliOverrides;
use crate::error::{OmniDocError, Result};
use crate::project_tools;
use crate::utils::path;
use serde::Serialize;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Serialize)]
struct DoctorCheck {
    name: String,
    ok: bool,
    detail: String,
}

pub fn handle_doctor(path: Option<String>, json: bool, strict: bool) -> Result<()> {
    let project_path = path::determine_project_path(path)?.canonicalize()?;
    let config_manager = create_config_manager_default(Some(&project_path))?;
    let config = config_manager.get_merged().clone();
    let mut checks = Vec::new();
    let executor = BuildExecutor::new(config.tool_paths.clone());
    let entry_is_latex = config
        .from
        .as_deref()
        .is_some_and(|format| format.eq_ignore_ascii_case("latex"))
        || config.entry.as_deref().is_some_and(|entry| {
            Path::new(entry)
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("tex"))
        });
    let outputs = if config.outputs.is_empty() {
        vec![config.to.clone().unwrap_or_else(|| "pdf".to_string())]
    } else {
        config.outputs.clone()
    };
    let has_pdf = outputs
        .iter()
        .any(|output| output.eq_ignore_ascii_case("pdf"));
    let has_epub = outputs.iter().any(|output| {
        matches!(
            output.trim().to_ascii_lowercase().as_str(),
            "epub" | "epub2" | "epub3"
        )
    });
    if !entry_is_latex {
        checks.push(doctor_tool(&executor, "pandoc", "pandoc"));
        checks.push(doctor_tool(&executor, "pandoc-crossref", "pandoc-crossref"));
    }
    if has_pdf {
        checks.push(doctor_tool(&executor, "latex_engine", "latex-engine"));
        let engine = executor.check_tool("latex_engine").ok();
        let tectonic = engine.as_deref().is_some_and(|engine| {
            Path::new(engine)
                .file_stem()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.eq_ignore_ascii_case("tectonic"))
        });
        if entry_is_latex && config.latex_backend.eq_ignore_ascii_case("latexmk") && !tectonic {
            checks.push(doctor_tool(&executor, "latexmk", "latexmk"));
        }
    }
    if has_epub {
        checks.push(doctor_tool(&executor, "epubcheck", "epubcheck"));
    }

    if let Some(lib_path) = &config.lib_path {
        let (ok, detail) = library_diagnostic(Path::new(lib_path));
        checks.push(DoctorCheck {
            name: "omnidoc-libs".to_string(),
            ok,
            detail,
        });
        if let Some(theme) = config.theme_name.as_deref() {
            let (ok, detail) = theme_diagnostic(Path::new(lib_path), theme, has_pdf);
            checks.push(DoctorCheck {
                name: format!("theme:{theme}"),
                ok,
                detail,
            });
        }
    } else {
        checks.push(DoctorCheck {
            name: "omnidoc-libs".to_string(),
            ok: false,
            detail: "library path is not configured".to_string(),
        });
        if let Some(theme) = config.theme_name.as_deref() {
            checks.push(DoctorCheck {
                name: format!("theme:{theme}"),
                ok: false,
                detail: "theme cannot be resolved without a configured library path".to_string(),
            });
        }
    }

    let issues = project_tools::validate_config(&project_path, &config);
    checks.push(DoctorCheck {
        name: "config".to_string(),
        ok: !project_tools::has_errors(&issues),
        detail: if issues.is_empty() {
            "valid".to_string()
        } else {
            issues
                .iter()
                .map(|issue| issue.message.as_str())
                .collect::<Vec<_>>()
                .join("; ")
        },
    });
    let failed = checks.iter().filter(|check| !check.ok).count();

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

    if strict && failed > 0 {
        return Err(OmniDocError::Project(format!(
            "environment diagnostics failed: {failed} check(s) failed"
        )));
    }

    Ok(())
}

fn doctor_tool(executor: &BuildExecutor, key: &str, name: &str) -> DoctorCheck {
    match executor.check_tool(key) {
        Ok(path) => {
            let version = Command::new(&path)
                .arg("--version")
                .output()
                .ok()
                .filter(|output| output.status.success())
                .and_then(|output| {
                    String::from_utf8_lossy(&output.stdout)
                        .lines()
                        .next()
                        .map(str::trim)
                        .filter(|line| !line.is_empty())
                        .map(str::to_string)
                });
            DoctorCheck {
                name: name.to_string(),
                ok: true,
                detail: version
                    .map(|version| format!("{} ({version})", path))
                    .unwrap_or(path),
            }
        }
        Err(error) => DoctorCheck {
            name: name.to_string(),
            ok: false,
            detail: error.to_string(),
        },
    }
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
        for file in &graph.files {
            println!("{}", file);
        }
        for resource in &graph.resources {
            println!(
                "resource {} [{}] {}",
                resource.logical_name, resource.resolved_from, resource.path
            );
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
    let base_manager = create_config_manager_default(Some(&project_path))?;
    let base_config = base_manager.get_merged();
    let outputs = if base_config.outputs.is_empty() {
        vec![base_config.to.clone().unwrap_or_else(|| "pdf".to_string())]
    } else {
        base_config.outputs.clone()
    };
    let targets = outputs
        .into_iter()
        .map(|output| {
            let manager = create_config_manager(
                Some(&project_path),
                CliOverrides::new().with_to(Some(output.clone())),
            )?;
            let config = manager.get_merged().clone();
            let graph = project_tools::dependency_graph(&project_path, &config);
            Ok((output, config, graph))
        })
        .collect::<Result<Vec<_>>>()?;
    let inputs = targets
        .iter()
        .map(|(output, config, graph)| project_tools::LockTargetInput {
            output,
            config,
            graph,
        })
        .collect::<Vec<_>>();
    if check {
        let status = project_tools::check_lock_targets(&project_path, &inputs)?;
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
    let _project_lock =
        project_tools::acquire_project_write_lock(&project_path, "update lock file")?;
    project_tools::write_lock_targets(&project_path, &inputs)?;
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
