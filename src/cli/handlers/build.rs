use crate::cli::handlers::common::{
    check_omnidoc_project, create_build_service, create_config_manager,
};
use crate::config::CliOverrides;
use crate::error::{OmniDocError, Result};
use crate::project_tools;
use crate::utils::path;
use std::path::Path;
use std::time::Instant;

#[derive(Debug, Clone, Default)]
pub struct BuildRunOptions {
    pub force: bool,
    pub report: bool,
    pub write_lock: bool,
    pub strict: bool,
}

/// Handle the 'build' command
#[allow(clippy::too_many_arguments)]
pub fn handle_build(
    path: Option<String>,
    to: Option<String>,
    all: bool,
    outputs: Vec<String>,
    pdf_engine: Option<String>,
    latex_backend: String,
    max_latex_passes: Option<usize>,
    force: bool,
    report: bool,
    write_lock: bool,
    strict: bool,
    verbose: bool,
) -> Result<()> {
    let project_path = path::determine_project_path(path)?;
    let project_path = project_path.canonicalize()?;
    let cli_overrides = build_cli_overrides(
        to,
        outputs,
        pdf_engine,
        latex_backend,
        max_latex_passes,
        verbose,
    );

    build_project_outputs(
        &project_path,
        cli_overrides,
        all,
        BuildRunOptions {
            force,
            report,
            write_lock,
            strict,
        },
        verbose,
    )
}

pub fn build_project(
    project_path: &Path,
    cli_overrides: CliOverrides,
    verbose: bool,
) -> Result<()> {
    build_project_once(
        project_path,
        cli_overrides,
        BuildRunOptions::default(),
        verbose,
    )
    .map(|_| ())
}

pub fn build_project_outputs(
    project_path: &Path,
    cli_overrides: CliOverrides,
    all: bool,
    run_options: BuildRunOptions,
    verbose: bool,
) -> Result<()> {
    let config_manager = create_config_manager(Some(project_path), cli_overrides.clone())?;
    let merged = config_manager.get_merged().clone();
    let outputs = resolve_outputs(&merged, &cli_overrides, all);
    let mut reports = Vec::new();
    let mut per_output_options = run_options.clone();
    per_output_options.write_lock = false;

    if outputs.len() == 1 {
        let report = build_project_once(
            project_path,
            cli_overrides.clone().with_to(outputs.into_iter().next()),
            per_output_options.clone(),
            verbose,
        )?;
        reports.push(report);
    } else if outputs.is_empty() {
        let report = build_project_once(
            project_path,
            cli_overrides.clone(),
            per_output_options.clone(),
            verbose,
        )?;
        reports.push(report);
    } else {
        for output in outputs {
            let output_overrides = cli_overrides.clone().with_to(Some(output));
            reports.push(build_project_once(
                project_path,
                output_overrides,
                per_output_options.clone(),
                verbose,
            )?);
        }
    }

    if run_options.report {
        project_tools::write_reports(project_path, &merged, &reports)?;
    }
    if run_options.write_lock {
        write_lock_for_reports(project_path, &cli_overrides, &reports)?;
    }

    Ok(())
}

fn build_project_once(
    project_path: &Path,
    cli_overrides: CliOverrides,
    run_options: BuildRunOptions,
    verbose: bool,
) -> Result<project_tools::BuildReport> {
    let started_at = Instant::now();
    check_omnidoc_project(project_path)?;

    let config_manager = create_config_manager(Some(project_path), cli_overrides.clone())?;
    let config = config_manager.get_merged().clone();
    config_manager.setup_env()?;
    let asset_context = project_tools::PluginContext {
        project_path,
        config: &config,
        output: None,
        target: None,
    };
    project_tools::run_plugin_hook(&asset_context, project_tools::PluginHook::AssetProvider)?;

    let mut issues = project_tools::validate_config(project_path, &config);
    issues.extend(project_tools::lint_project(project_path));
    issues.extend(project_tools::run_plugin_lint_rules(project_path, &config));
    if run_options.strict && project_tools::has_warnings_or_errors(&issues) {
        project_tools::print_issues(&issues);
        return Err(OmniDocError::Project(
            "Strict mode failed because lint/config issues were found".to_string(),
        ));
    }
    if project_tools::has_errors(&issues) {
        project_tools::print_issues(&issues);
        return Err(OmniDocError::Project(
            "Configuration validation failed".to_string(),
        ));
    }

    let graph = project_tools::dependency_graph(project_path, &config);
    let output = config.to.clone().unwrap_or_else(|| "pdf".to_string());
    let target = config.target.clone().unwrap_or_else(|| {
        project_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("document")
            .to_string()
    });
    let input_digest = project_tools::build_input_digest(project_path, &graph, &config, &output)?;

    let output_file = expected_output_file(project_path, &config, &output, &target);
    if !run_options.force
        && output_file.exists()
        && project_tools::cache_hit(project_path, &output, &input_digest)
    {
        if verbose {
            println!("Skipping {} build; input cache is unchanged.", output);
        }
        return Ok(project_tools::build_report(
            project_tools::BuildReportContext {
                output,
                target,
                skipped: true,
                cache_reason: "input_digest_match".to_string(),
                duration_ms: started_at.elapsed().as_millis() as u64,
                input_digest,
                graph: &graph,
                config: &config,
                artifact: &output_file,
                issues,
            },
        ));
    }

    let cache_reason = if run_options.force {
        "forced_rebuild"
    } else if !output_file.exists() {
        "artifact_missing"
    } else {
        "input_digest_changed"
    };

    let build_context = project_tools::PluginContext {
        project_path,
        config: &config,
        output: Some(&output),
        target: Some(&target),
    };
    project_tools::run_plugin_hook(&build_context, project_tools::PluginHook::PreBuild)?;

    let build_service = create_build_service(Some(project_path), cli_overrides)?;
    build_service
        .build(project_path, verbose)
        .map_err(|e| OmniDocError::Project(format!("Failed to build project: {}", e)))?;
    project_tools::run_plugin_hook(&build_context, project_tools::PluginHook::PostBuild)?;

    project_tools::write_cache(project_path, &output, &input_digest)?;
    Ok(project_tools::build_report(
        project_tools::BuildReportContext {
            output,
            target,
            skipped: false,
            cache_reason: cache_reason.to_string(),
            duration_ms: started_at.elapsed().as_millis() as u64,
            input_digest,
            graph: &graph,
            config: &config,
            artifact: &output_file,
            issues,
        },
    ))
}

fn write_lock_for_reports(
    project_path: &Path,
    cli_overrides: &CliOverrides,
    reports: &[project_tools::BuildReport],
) -> Result<()> {
    let targets = reports
        .iter()
        .map(|report| {
            let config_manager = create_config_manager(
                Some(project_path),
                cli_overrides.clone().with_to(Some(report.output.clone())),
            )?;
            let config = config_manager.get_merged().clone();
            let graph = project_tools::dependency_graph(project_path, &config);
            Ok((report.output.clone(), config, graph))
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
    project_tools::write_lock_targets(project_path, &inputs)
}

pub fn build_cli_overrides(
    to: Option<String>,
    outputs: Vec<String>,
    pdf_engine: Option<String>,
    latex_backend: String,
    max_latex_passes: Option<usize>,
    verbose: bool,
) -> CliOverrides {
    let mut cli_overrides = CliOverrides::new()
        .with_verbose(verbose)
        .with_to(to)
        .with_outputs(outputs)
        .with_latex_backend(Some(latex_backend))
        .with_max_latex_passes(max_latex_passes);
    if let Some(engine) = pdf_engine {
        cli_overrides = cli_overrides.with_tool_path("latex_engine".to_string(), Some(engine));
    }
    cli_overrides
}

pub(crate) fn resolve_outputs(
    config: &crate::config::MergedConfig,
    cli_overrides: &CliOverrides,
    all: bool,
) -> Vec<String> {
    if !cli_overrides.outputs.is_empty() {
        return normalize_outputs(cli_overrides.outputs.clone());
    }
    if let Some(to) = &cli_overrides.to {
        return normalize_outputs(vec![to.clone()]);
    }
    if all {
        if !config.outputs.is_empty() {
            return normalize_outputs(config.outputs.clone());
        }
        return project_tools::default_all_outputs();
    }
    normalize_outputs(vec![config.to.clone().unwrap_or_else(|| "pdf".to_string())])
}

fn normalize_outputs(outputs: Vec<String>) -> Vec<String> {
    let mut normalized = Vec::new();
    for output in outputs {
        let output = output.trim().to_ascii_lowercase();
        if output.is_empty() || normalized.iter().any(|item| item == &output) {
            continue;
        }
        normalized.push(output);
    }
    normalized
}

pub(crate) fn expected_output_file(
    project_path: &Path,
    config: &crate::config::MergedConfig,
    output: &str,
    target: &str,
) -> std::path::PathBuf {
    let outdir = config
        .outdir
        .as_ref()
        .map(|outdir| project_path.join(outdir))
        .unwrap_or_else(|| project_path.join("build"));
    let extension = match output {
        "latex" => "tex",
        other => other,
    };
    outdir.join(format!("{}.{}", target, extension))
}

#[cfg(test)]
mod tests {
    use super::resolve_outputs;
    use crate::config::{CliOverrides, MergedConfig};

    #[test]
    fn plain_build_uses_project_to_even_when_all_outputs_are_configured() {
        let config = MergedConfig {
            to: Some("html".to_string()),
            outputs: vec!["pdf".to_string(), "docx".to_string()],
            ..Default::default()
        };

        let outputs = resolve_outputs(&config, &CliOverrides::new(), false);

        assert_eq!(outputs, vec!["html"]);
    }

    #[test]
    fn cli_to_overrides_configured_outputs() {
        let config = MergedConfig {
            to: Some("pdf".to_string()),
            outputs: vec!["pdf".to_string(), "docx".to_string()],
            ..Default::default()
        };
        let cli = CliOverrides::new().with_to(Some("epub".to_string()));

        let outputs = resolve_outputs(&config, &cli, true);

        assert_eq!(outputs, vec!["epub"]);
    }

    #[test]
    fn cli_outputs_have_highest_priority() {
        let config = MergedConfig {
            to: Some("pdf".to_string()),
            outputs: vec!["pdf".to_string()],
            ..Default::default()
        };
        let cli = CliOverrides::new()
            .with_to(Some("html".to_string()))
            .with_outputs(vec!["docx".to_string(), "epub".to_string()]);

        let outputs = resolve_outputs(&config, &cli, true);

        assert_eq!(outputs, vec!["docx", "epub"]);
    }

    #[test]
    fn all_uses_configured_outputs_or_defaults() {
        let config = MergedConfig {
            outputs: vec!["PDF".to_string(), "pdf".to_string(), "html".to_string()],
            ..Default::default()
        };

        let configured = resolve_outputs(&config, &CliOverrides::new(), true);
        let defaults = resolve_outputs(&MergedConfig::default(), &CliOverrides::new(), true);

        assert_eq!(configured, vec!["pdf", "html"]);
        assert_eq!(defaults, vec!["pdf", "html", "docx", "epub"]);
    }
}
