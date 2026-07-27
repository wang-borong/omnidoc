use crate::cli::handlers::common::{create_config_manager_default, merged_config_to_envs};
use crate::cli::utils::{infer_title, resolve_creation_template};
use crate::config::ProjectConfig;
use crate::doc::Doc;
use crate::doctype::DocumentFormat;
use crate::error::{OmniDocError, Result};
use crate::utils::fs;
use std::env;
use std::path::Path;

/// Handle the 'new' command
pub fn handle_new(
    orig_path: &std::path::Path,
    path: String,
    title: Option<String>,
    author: Option<String>,
    doctype: Option<String>,
    format: Option<DocumentFormat>,
    defaults: bool,
) -> Result<()> {
    let requested_path = Path::new(&path);
    let target_path = if requested_path.is_absolute() {
        requested_path.to_path_buf()
    } else {
        orig_path.join(requested_path)
    };
    if fs::exists(&target_path) {
        return Err(OmniDocError::Project(format!(
            "The path already exists: {}",
            target_path.display()
        )));
    }

    let config_manager = create_config_manager_default(None)?;
    let merged_config = config_manager.get_merged();
    let envs = merged_config_to_envs(merged_config);
    let author = author
        .or_else(|| merged_config.author.clone())
        .unwrap_or_else(|| "Someone".to_string());
    let template = resolve_creation_template(doctype, format, defaults)?;
    let title = title.unwrap_or_else(|| infer_title(&target_path));

    fs::create_dir_all(&target_path)?;
    let target_path = target_path.canonicalize().map_err(|error| {
        let _ = fs::remove_dir_all(&target_path);
        OmniDocError::Io(error)
    })?;
    env::set_current_dir(&target_path).map_err(|error| {
        let _ = env::set_current_dir(orig_path);
        let _ = fs::remove_dir_all(&target_path);
        OmniDocError::Io(error)
    })?;

    let path_string = target_path.to_string_lossy().to_string();
    let doc = Doc::new(&title, &path_string, &author, &template.key, envs);
    let result: Result<()> = (|| {
        let target_name = target_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("document");
        ProjectConfig::create_default(
            &target_path,
            Some(&template.file_name),
            Some(template.format.as_str()),
            Some("pdf"),
            Some(target_name),
        )?;
        doc.create_project()?;
        Ok(())
    })();

    if let Err(error) = result {
        let _ = env::set_current_dir(orig_path);
        let _ = fs::remove_dir_all(&target_path);
        return Err(OmniDocError::Project(format!(
            "Failed to create project: {}",
            error
        )));
    }

    println!(
        "✓ Ready: '{}' ({}, {})",
        target_path.display(),
        template.key,
        template.format.as_str()
    );
    println!("  Next: cd {:?} && omnidoc build", target_path);
    let _ = env::set_current_dir(orig_path);

    Ok(())
}
