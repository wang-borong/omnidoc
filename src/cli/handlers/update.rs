use crate::config::ConfigParser;
use crate::constants::paths_internal;
use crate::doc::Doc;
use crate::error::{OmniDocError, Result};

/// Handle the 'update' command
pub fn handle_update(path: Option<String>) -> Result<()> {
    let config_parser = ConfigParser::default()
        .map_err(|e| OmniDocError::Config(format!("Failed to load config: {}", e)))?;
    let envs = config_parser
        .get_envs()
        .map_err(|e| OmniDocError::Config(format!("Failed to get envs: {}", e)))?;

    let path = path.unwrap_or_else(|| paths_internal::CURRENT_DIR.to_string());

    let mut doc = Doc::new("", &path, "", "", envs);

    doc.update_project()
        .map_err(|e| OmniDocError::Project(format!("Update project failed: {}", e)))?;

    Ok(())
}
