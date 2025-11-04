use crate::config::{CliOverrides, ConfigManager};
use crate::doc::services::BuildService;
use crate::doc::is_omnidoc_project;
use crate::error::{OmniDocError, Result};
use std::path::Path;

/// Handle the 'clean' command
pub fn handle_clean(path: Option<String>, distclean: bool) -> Result<()> {
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

    // 创建配置管理器
    let cli_overrides = CliOverrides::new();
    let config_manager = ConfigManager::new(Some(&project_path), cli_overrides)
        .map_err(|e| OmniDocError::Config(format!("Failed to load config: {}", e)))?;

    // 创建构建服务
    let merged_config = config_manager.get_merged().clone();
    let build_service = BuildService::new(merged_config);

    // 执行清理
    build_service.clean(&project_path, distclean)
        .map_err(|e| OmniDocError::Project(format!("Failed to clean project: {}", e)))?;

    Ok(())
}
