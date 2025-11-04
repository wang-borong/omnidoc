use crate::cli::handlers::common::{create_config_manager_default, merged_config_to_envs};
use crate::cli::utils::get_doctype_from_readline;
use crate::config::{CliOverrides, ProjectConfig};
use crate::constants::paths_internal;
use crate::doc::Doc;
use crate::doctype::DocumentTypeRegistry;
use crate::error::{OmniDocError, Result};
use crate::utils::fs;
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
    if fs::exists(&path) {
        return Err(OmniDocError::Project(format!(
            "The path already exists: {}",
            path
        )));
    }

    fs::create_dir_all(&path)?;
    env::set_current_dir(&path).map_err(|e| {
        let _ = env::set_current_dir(orig_path);
        let _ = fs::remove_dir_all(&path);
        OmniDocError::Io(e)
    })?;

    // Load config and get envs
    let config_manager = create_config_manager_default(None).map_err(|e| {
        let _ = env::set_current_dir(orig_path);
        let _ = fs::remove_dir_all(&path);
        e
    })?;

    let merged_config = config_manager.get_merged();
    let envs = merged_config_to_envs(merged_config);

    let author = author
        .or_else(|| merged_config.author.clone())
        .unwrap_or_else(|| "Someone".to_string());

    // Get document type from user
    let doctype_str = get_doctype_from_readline(orig_path, &path)?;

    // Create the project
    let doc = Doc::new(&title, &path, &author, &doctype_str, envs);
    doc.create_project().map_err(|e| {
        let _ = env::set_current_dir(paths_internal::PARENT_DIR);
        let _ = fs::remove_dir_all(&path);
        OmniDocError::Project(format!("Failed to create project: {}", e))
    })?;

    // 创建项目配置文件
    let project_path = Path::new(&path);
    let doctype = DocumentTypeRegistry::from_str(&doctype_str)
        .map_err(|e| OmniDocError::Project(format!("Invalid document type: {}", e)))?;

    let entry = Some(doctype.file_name());
    let from = if doctype.file_extension() == "md" {
        Some("markdown")
    } else {
        Some("latex")
    };
    let to = Some("pdf");
    let target_name = project_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("document");

    ProjectConfig::create_default(project_path, entry, from, to, Some(target_name)).map_err(
        |e| {
            eprintln!("Warning: Failed to create project config: {}", e);
            e
        },
    )?;

    println!(
        "✓ Created project configuration file: {}/.omnidoc.toml",
        path
    );

    Ok(())
}
