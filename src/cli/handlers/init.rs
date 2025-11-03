use crate::cli::utils::get_doctype_from_readline;
use crate::config::ConfigParser;
use crate::constants::paths_internal;
use crate::doc::Doc;
use crate::error::{OmniDocError, Result};
use std::path::Path;

/// Handle the 'init' command
pub fn handle_init(
    orig_path: &std::path::Path,
    path: Option<String>,
    title: String,
    author: Option<String>,
) -> Result<()> {
    let path = path.unwrap_or_else(|| paths_internal::CURRENT_DIR.to_string());

    // Load config and get envs
    let config_parser = ConfigParser::default()
        .map_err(|e| OmniDocError::Config(format!("Failed to load config: {}", e)))?;
    let envs = config_parser
        .get_envs()
        .map_err(|e| OmniDocError::Config(format!("Failed to get envs: {}", e)))?;

    let author = author
        .or_else(|| config_parser.get_author_name().ok())
        .unwrap_or_else(|| "Someone".to_string());

    // Get document type from user
    let doctype_str = get_doctype_from_readline(orig_path, &path)?;

    // Initialize the project
    let doc = Doc::new(&title, &path, &author, &doctype_str, envs);
    if Doc::is_omnidoc_project() {
        return Err(OmniDocError::Project(
            "It is an omnidoc project already, no action".to_string(),
        ));
    }
    doc.init_project(false)
        .map_err(|e| OmniDocError::Project(format!("Initial project failed: {}", e)))?;

    Ok(())
}
