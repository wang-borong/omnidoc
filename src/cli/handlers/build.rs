use crate::cli::handlers::common::{
    check_omnidoc_project, create_build_service, create_config_manager,
};
use crate::config::CliOverrides;
use crate::error::{OmniDocError, Result};
use crate::utils::path;

/// Handle the 'build' command
pub fn handle_build(path: Option<String>, verbose: bool) -> Result<()> {
    let project_path = path::determine_project_path(path)?;
    check_omnidoc_project(&project_path)?;

    let cli_overrides = CliOverrides::new().with_verbose(verbose);
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
