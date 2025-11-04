use crate::config::ConfigParser;
use crate::constants::paths_internal;
use crate::doc::Doc;
use crate::error::{OmniDocError, Result};

/// Handle the 'clean' command
pub fn handle_clean(path: Option<String>, distclean: bool) -> Result<()> {
    let config_parser = ConfigParser::default()
        .map_err(|e| OmniDocError::Config(format!("Failed to load config: {}", e)))?;
    let envs = config_parser.get_envs().map_err(|e| {
        OmniDocError::Config(format!("Failed to retrieve environment variables: {}", e))
    })?;

    let path = path.unwrap_or_else(|| paths_internal::CURRENT_DIR.to_string());
    let doc = Doc::new("", &path, "", "", envs);

    doc.clean_project(distclean)
        .map_err(|e| OmniDocError::Project(format!("Failed to clean project: {}", e)))?;

    Ok(())
}
