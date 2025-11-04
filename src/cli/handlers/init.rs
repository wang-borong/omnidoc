use crate::cli::utils::get_doctype_from_readline;
use crate::config::{ConfigParser, ProjectConfig};
use crate::constants::paths_internal;
use crate::doc::Doc;
use crate::doctype::DocumentTypeRegistry;
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
    let envs = config_parser.get_envs().map_err(|e| {
        OmniDocError::Config(format!("Failed to retrieve environment variables: {}", e))
    })?;

    let author = author
        .or_else(|| config_parser.get_author_name().ok())
        .unwrap_or_else(|| "Someone".to_string());

    // Get document type from user
    let doctype_str = get_doctype_from_readline(orig_path, &path)?;

    // Initialize the project
    let doc = Doc::new(&title, &path, &author, &doctype_str, envs);
    if Doc::is_omnidoc_project() {
        return Err(OmniDocError::Project(
            "This is already an OmniDoc project, no action taken".to_string(),
        ));
    }
    doc.init_project(false)
        .map_err(|e| OmniDocError::Project(format!("Failed to initialize project: {}", e)))?;

    // 创建项目配置文件（如果不存在）
    let project_path = Path::new(&path);
    if !ProjectConfig::exists(project_path) {
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
        
        ProjectConfig::create_default(project_path, entry, from, to, Some(target_name))
            .map_err(|e| {
                eprintln!("Warning: Failed to create project config: {}", e);
                e
            })?;

        println!("✓ Created project configuration file: .omnidoc.toml");
    }

    Ok(())
}
