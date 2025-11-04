use crate::config::{CliOverrides, ConfigManager};
use crate::doc::is_omnidoc_project;
use crate::doc::services::BuildService;
use crate::error::{OmniDocError, Result};
use std::path::Path;

/// Handle the 'build' command
pub fn handle_build(path: Option<String>, verbose: bool) -> Result<()> {
    // 确定项目路径
    let project_path = if let Some(p) = path {
        Path::new(&p).to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| OmniDocError::Io(e))?
    };

    // 检查是否是 omnidoc 项目
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(&project_path)?;
    let is_project = is_omnidoc_project();
    std::env::set_current_dir(original_dir)?;

    if !is_project {
        return Err(OmniDocError::NotOmniDocProject(
            format!("The directory '{}' is not an OmniDoc project", project_path.display())
        ));
    }

    // 创建配置管理器（包含 CLI 覆盖）
    let cli_overrides = CliOverrides::new().with_verbose(verbose);
    let config_manager = ConfigManager::new(Some(&project_path), cli_overrides)
        .map_err(|e| OmniDocError::Config(format!("Failed to load config: {}", e)))?;

    // 设置环境变量
    config_manager.setup_env()?;

    // 创建构建服务
    let merged_config = config_manager.get_merged().clone();
    let build_service = BuildService::new(merged_config);

    // 执行构建
    build_service.build(&project_path, verbose)
        .map_err(|e| OmniDocError::Project(format!("Failed to build project: {}", e)))?;

    Ok(())
}
