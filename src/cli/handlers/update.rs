use crate::build::pipeline::{detect_project_type, ProjectType};
use crate::cli::handlers::common::{
    create_config_manager_default, merged_config_to_envs, print_json_error,
};
use crate::doc::{Doc, ProjectUpdateAction};
use crate::error::{OmniDocError, Result};
use crate::utils::path;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize)]
struct UpdateReport {
    schema_version: u32,
    project_root: String,
    dry_run: bool,
    commit: bool,
    applied: bool,
    actions: Vec<ProjectUpdateAction>,
}

/// Handle the 'update' command
pub fn handle_update(
    path: Option<String>,
    dry_run: bool,
    no_commit: bool,
    json: bool,
) -> Result<()> {
    let report = match update_project(path, dry_run, !no_commit) {
        Ok(report) => report,
        Err(error) => {
            if json {
                print_json_error(&error);
            }
            return Err(error);
        }
    };

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|error| {
                OmniDocError::Other(format!("Failed to serialize update report: {error}"))
            })?
        );
    } else {
        print_update_report(&report);
    }

    Ok(())
}

fn update_project(path: Option<String>, dry_run: bool, commit: bool) -> Result<UpdateReport> {
    let project_path = path::determine_project_root(path)?;
    let _working_directory = path::WorkingDirectoryGuard::enter(&project_path)?;

    let config_manager = create_config_manager_default(Some(project_path.as_path()))?;
    let merged_config = config_manager.get_merged();
    let envs = merged_config_to_envs(merged_config);
    let template_key = match detect_project_type(merged_config, &project_path) {
        ProjectType::Latex => "ctexart-tex",
        ProjectType::Markdown | ProjectType::Unknown => "ctex-md",
    };

    let path_str = project_path.to_string_lossy().to_string();
    let mut doc = Doc::new("", &path_str, "", template_key, envs);
    let actions = doc.plan_update(commit)?;

    if !dry_run {
        let _project_lock =
            crate::project_tools::acquire_project_write_lock(&project_path, "update project")?;
        doc.update_project_with_options(commit, false)?;
    }

    Ok(UpdateReport {
        schema_version: 1,
        project_root: path_str,
        dry_run,
        commit,
        applied: !dry_run,
        actions,
    })
}

fn print_update_report(report: &UpdateReport) {
    let project_root = Path::new(&report.project_root);
    if report.dry_run {
        println!("Update plan for {}:", project_root.display());
    } else {
        println!("Updated {}:", project_root.display());
    }

    for action in &report.actions {
        let verb = if report.dry_run { "Would" } else { "Did" };
        let path = display_path(project_root, &action.path);
        match (action.operation.as_str(), action.destination.as_deref()) {
            ("refresh_file", _) => println!("  {verb} refresh {path}"),
            ("create_directory", _) => println!("  {verb} create directory {path}"),
            ("move_file", Some(destination)) => println!(
                "  {verb} move {path} -> {}",
                display_path(project_root, destination)
            ),
            ("initialize_git", _) => println!("  {verb} initialize Git repository at {path}"),
            ("commit", _) => println!("  {verb} create an update commit"),
            (operation, _) => println!("  {verb} {operation} {path}"),
        }
    }

    if report.dry_run {
        println!("No files were changed.");
    } else if !report.commit {
        println!("Changes were left uncommitted (--no-commit).");
    }
}

fn display_path(project_root: &Path, path: &str) -> String {
    let path = Path::new(path);
    match path.strip_prefix(project_root) {
        Ok(relative) if relative.as_os_str().is_empty() => ".".to_string(),
        Ok(relative) => relative.display().to_string(),
        Err(_) => path.display().to_string(),
    }
}
