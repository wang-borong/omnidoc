use crate::build::pipeline::{detect_project_type, ProjectType};
use crate::cli::handlers::common::{create_config_manager_default, merged_config_to_envs};
use crate::doc::Doc;
use crate::error::Result;
use crate::utils::path;

/// Handle the 'update' command
pub fn handle_update(path: Option<String>) -> Result<()> {
    let project_path = path::determine_project_path(path)?;

    let config_manager = create_config_manager_default(Some(project_path.as_path()))?;
    let merged_config = config_manager.get_merged();
    let envs = merged_config_to_envs(merged_config);
    let template_key = match detect_project_type(merged_config, &project_path) {
        ProjectType::Latex => "ctexart-tex",
        ProjectType::Markdown | ProjectType::Unknown => "ctex-md",
    };

    let path_str = project_path.to_string_lossy().to_string();
    let mut doc = Doc::new("", &path_str, "", template_key, envs);

    doc.update_project()?;

    Ok(())
}
