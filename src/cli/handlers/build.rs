use crate::cli::handlers::common::{
    check_omnidoc_project, create_build_service, create_config_manager,
};
use crate::config::CliOverrides;
use crate::error::{OmniDocError, Result};
use crate::utils::path;
use std::path::Path;

/// Handle the 'build' command
pub fn handle_build(
    path: Option<String>,
    to: Option<String>,
    pdf_engine: Option<String>,
    latex_backend: String,
    max_latex_passes: Option<usize>,
    verbose: bool,
) -> Result<()> {
    let project_path = path::determine_project_path(path)?;
    let project_path = project_path.canonicalize()?;
    let cli_overrides =
        build_cli_overrides(to, pdf_engine, latex_backend, max_latex_passes, verbose);

    build_project(&project_path, cli_overrides, verbose)
}

pub fn build_project(
    project_path: &Path,
    cli_overrides: CliOverrides,
    verbose: bool,
) -> Result<()> {
    check_omnidoc_project(&project_path)?;

    let build_service = create_build_service(Some(&project_path), cli_overrides.clone())?;

    // 设置环境变量
    let config_manager = create_config_manager(Some(&project_path), cli_overrides)?;
    config_manager.setup_env()?;

    // 执行构建
    build_service
        .build(&project_path, verbose)
        .map_err(|e| OmniDocError::Project(format!("Failed to build project: {}", e)))?;

    Ok(())
}

pub fn build_cli_overrides(
    to: Option<String>,
    pdf_engine: Option<String>,
    latex_backend: String,
    max_latex_passes: Option<usize>,
    verbose: bool,
) -> CliOverrides {
    let mut cli_overrides = CliOverrides::new()
        .with_verbose(verbose)
        .with_to(to)
        .with_latex_backend(Some(latex_backend))
        .with_max_latex_passes(max_latex_passes);
    if let Some(engine) = pdf_engine {
        cli_overrides = cli_overrides.with_tool_path("latex_engine".to_string(), Some(engine));
    }
    cli_overrides
}
