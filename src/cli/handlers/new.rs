use crate::cli::utils::get_doctype_from_readline;
use crate::config::ConfigParser;
use crate::constants::paths_internal;
use crate::doc::Doc;
use crate::error::{OmniDocError, Result};
use crate::fs;
use std::env;
use std::path::Path;

/// Handle the 'new' command
pub fn handle_new(
    orig_path: &std::path::Path,
    path: String,
    title: String,
    author: Option<String>,
) -> Result<()> {
    // Create directory and change to it
    if Path::new(&path).exists() {
        return Err(OmniDocError::Project(format!(
            "The path already exists: {}",
            path
        )));
    }

    fs::create_dir_all(&path).map_err(|e| OmniDocError::Io(e))?;
    env::set_current_dir(&path).map_err(|e| {
        let _ = env::set_current_dir(orig_path);
        let _ = fs::remove_dir_all(&path);
        OmniDocError::Io(e)
    })?;

    // Load config and get envs
    let config_parser = ConfigParser::default().map_err(|e| {
        let _ = env::set_current_dir(orig_path);
        let _ = fs::remove_dir_all(&path);
        OmniDocError::Config(format!("Failed to load config: {}", e))
    })?;

    let envs = config_parser.get_envs().map_err(|e| {
        let _ = env::set_current_dir(orig_path);
        let _ = fs::remove_dir_all(&path);
        OmniDocError::Config(format!("Failed to get envs: {}", e))
    })?;

    let author = author
        .or_else(|| config_parser.get_author_name().ok())
        .unwrap_or_else(|| "Someone".to_string());

    // Get document type from user
    let doctype_str = get_doctype_from_readline(orig_path, &path)?;

    // Create the project
    let doc = Doc::new(&title, &path, &author, &doctype_str, envs);
    doc.create_project().map_err(|e| {
        let _ = env::set_current_dir(paths_internal::PARENT_DIR);
        let _ = fs::remove_dir_all(&path);
        OmniDocError::Project(format!("Create project failed: {}", e))
    })?;

    Ok(())
}
