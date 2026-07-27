use crate::cli::handlers::common::{
    create_config_manager_default, dirty_auto_commit_error, merged_config_to_envs, user_git_changes,
};
use crate::cli::utils::{infer_title, resolve_creation_template};
use crate::config::ProjectConfig;
use crate::constants::paths;
use crate::doc::Doc;
use crate::doctype::DocumentFormat;
use crate::error::{OmniDocError, Result};
use crate::git::{git_has_commits, is_git_repo};
use crate::utils::fs;
use std::env;

/// Handle the 'init' command
pub fn handle_init(
    title: Option<String>,
    author: Option<String>,
    doctype: Option<String>,
    format: Option<DocumentFormat>,
    defaults: bool,
    no_commit: bool,
) -> Result<()> {
    let project_path = env::current_dir().map_err(OmniDocError::Io)?;
    if ProjectConfig::exists(&project_path) {
        return Err(OmniDocError::Project(
            "This is already an OmniDoc project, no action taken".to_string(),
        ));
    }
    let commit = !no_commit;

    let config_manager = create_config_manager_default(Some(&project_path))?;
    let merged_config = config_manager.get_merged();
    let envs = merged_config_to_envs(merged_config);
    let author = author
        .or_else(|| merged_config.author.clone())
        .unwrap_or_else(|| "Someone".to_string());
    let template = resolve_creation_template(doctype, format, defaults)?;
    let title = title.unwrap_or_else(|| infer_title(&project_path));

    if commit && is_git_repo(&project_path) && git_has_commits(&project_path)? {
        let changes = user_git_changes(&project_path)?;
        if !changes.is_empty() {
            return Err(dirty_auto_commit_error(
                "an initialization commit",
                "--no-commit",
                &changes,
            ));
        }
    }

    let path_string = project_path.to_string_lossy().to_string();
    let doc = Doc::new(&title, &path_string, &author, &template.key, envs);
    let target_name = project_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("document");
    ProjectConfig::create_default(
        &project_path,
        Some(&template.file_name),
        Some(template.format.as_str()),
        Some("pdf"),
        Some(target_name),
    )?;
    if let Err(error) = doc.init_project_with_options(false, commit, true) {
        let _ = fs::remove_file(project_path.join(paths::PROJECT_CONFIG));
        return Err(OmniDocError::Project(format!(
            "Failed to initialize project: {}",
            error
        )));
    }

    println!("✓ Created project configuration file: .omnidoc.toml");
    println!(
        "✓ Ready: '{}' ({}, {})",
        project_path.display(),
        template.key,
        template.format.as_str()
    );
    println!("  Next: omnidoc build");
    if no_commit {
        println!("  Git changes were left uncommitted (--no-commit).");
    }

    Ok(())
}
