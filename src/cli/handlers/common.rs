use crate::config::{CliOverrides, ConfigManager, MergedConfig};
use crate::doc::services::{BuildService, ConverterService, FigureService};
use crate::error::Result;
use crate::utils::error;
use std::collections::HashMap;
use std::path::Path;

/// Helper to create ConfigManager and get merged config
pub fn create_config_manager(
    project_path: Option<&Path>,
    cli_overrides: CliOverrides,
) -> Result<ConfigManager> {
    error::config_err(
        ConfigManager::new(project_path, cli_overrides),
        "Failed to load config",
    )
}

/// Helper to create ConfigManager with default overrides
pub fn create_config_manager_default(project_path: Option<&Path>) -> Result<ConfigManager> {
    create_config_manager(project_path, CliOverrides::new())
}

/// Helper to get merged config from ConfigManager
pub fn get_merged_config(config_manager: &ConfigManager) -> MergedConfig {
    config_manager.get_merged().clone()
}

/// Helper to create BuildService
pub fn create_build_service(
    project_path: Option<&Path>,
    cli_overrides: CliOverrides,
) -> Result<BuildService> {
    let config_manager = create_config_manager(project_path, cli_overrides)?;
    let merged_config = get_merged_config(&config_manager);
    Ok(BuildService::new(merged_config))
}

/// Helper to create ConverterService
pub fn create_converter_service() -> Result<ConverterService> {
    let config_manager = create_config_manager_default(None)?;
    // 确保环境变量（如 TEXMFHOME）在转换前已设置
    let _ = config_manager.setup_env();
    let merged_config = get_merged_config(&config_manager);
    ConverterService::new(merged_config)
}

/// Helper to create FigureService with tool path overrides
pub fn create_figure_service(tool_overrides: Vec<(&str, Option<String>)>) -> Result<FigureService> {
    let mut cli_overrides = CliOverrides::new();
    let mut tool_paths = HashMap::new();

    for (tool, path) in &tool_overrides {
        cli_overrides = cli_overrides.with_tool_path(tool.to_string(), path.clone());
        tool_paths.insert(tool.to_string(), path.clone());
    }

    let config_manager = create_config_manager(None, cli_overrides)?;
    let mut merged_config = get_merged_config(&config_manager);
    merged_config.tool_paths.extend(tool_paths);

    FigureService::new(merged_config)
}

// Re-export path functions for backward compatibility
pub use crate::utils::path::check_omnidoc_project;

/// Convert MergedConfig to Doc's envs HashMap format
pub fn merged_config_to_envs(
    merged_config: &MergedConfig,
) -> HashMap<&'static str, Option<String>> {
    let mut envs = HashMap::new();
    envs.insert("outdir", merged_config.outdir.clone());
    envs.insert("texmfhome", merged_config.texmfhome.clone());
    envs.insert("texinputs", merged_config.texinputs.clone());
    envs.insert("bibinputs", merged_config.bibinputs.clone());
    envs
}
