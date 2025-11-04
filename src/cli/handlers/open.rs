use crate::config::ConfigParser;
use crate::constants::paths_internal;
use crate::doc::Doc;
use crate::error::{OmniDocError, Result};

/// Handle the 'open' command
pub fn handle_open(path: Option<String>) -> Result<()> {
    let config_parser = ConfigParser::default()
        .map_err(|e| OmniDocError::Config(format!("Failed to load config: {}", e)))?;
    let envs = config_parser.get_envs().map_err(|e| {
        OmniDocError::Config(format!("Failed to retrieve environment variables: {}", e))
    })?;

    let path = path.unwrap_or_else(|| paths_internal::CURRENT_DIR.to_string());
    let doc = Doc::new("", &path, "", "", envs);
    doc.open_doc()
        .map_err(|e| OmniDocError::Project(format!("Failed to open document: {}", e)))?;

    Ok(())
}
