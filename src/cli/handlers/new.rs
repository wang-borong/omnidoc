use crate::cli::handlers::common::{
    create_config_manager_default, merged_config_to_envs, print_json_error,
};
use crate::cli::utils::{infer_title, resolve_creation_template};
use crate::config::ProjectConfig;
use crate::doc::templates::ProjectTemplateInfo;
use crate::doc::{Doc, ProjectUpdateAction};
use crate::doctype::DocumentFormat;
use crate::error::{OmniDocError, Result};
use crate::utils::fs;
use serde::Serialize;
use std::env;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
struct NewProjectReport {
    schema_version: u32,
    project_root: String,
    title: String,
    author: String,
    template: ProjectTemplateInfo,
    dry_run: bool,
    commit: bool,
    applied: bool,
    actions: Vec<ProjectUpdateAction>,
}

/// Handle the `new` command.
#[allow(clippy::too_many_arguments)]
pub fn handle_new(
    orig_path: &Path,
    path: String,
    title: Option<String>,
    author: Option<String>,
    doctype: Option<String>,
    format: Option<DocumentFormat>,
    defaults: bool,
    dry_run: bool,
    no_commit: bool,
    json: bool,
) -> Result<()> {
    let report = match create_new_project(
        orig_path, path, title, author, doctype, format, defaults, dry_run, !no_commit,
    ) {
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
                OmniDocError::Other(format!(
                    "Failed to serialize project creation report: {error}"
                ))
            })?
        );
    } else {
        print_new_project_report(&report);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn create_new_project(
    orig_path: &Path,
    path: String,
    title: Option<String>,
    author: Option<String>,
    doctype: Option<String>,
    format: Option<DocumentFormat>,
    defaults: bool,
    dry_run: bool,
    commit: bool,
) -> Result<NewProjectReport> {
    let requested_path = Path::new(&path);
    let target_path = if requested_path.is_absolute() {
        requested_path.to_path_buf()
    } else {
        orig_path.join(requested_path)
    };
    if fs::exists(&target_path) {
        return Err(OmniDocError::Project(format!(
            "The target path already exists: {}\nUse `omnidoc init {}` to initialize an existing directory.",
            target_path.display(),
            requested_path.display()
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
    let target_string = target_path.to_string_lossy().to_string();
    let preview_doc = Doc::new(&title, &target_string, &author, &template.key, envs.clone());
    let preview_actions = preview_doc.plan_new(commit)?;

    if dry_run {
        return Ok(new_project_report(
            target_path,
            title,
            author,
            template,
            true,
            commit,
            false,
            preview_actions,
        ));
    }

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
    let actions = match doc.plan_new(commit) {
        Ok(actions) => actions,
        Err(error) => {
            let _ = env::set_current_dir(orig_path);
            let _ = fs::remove_dir_all(&target_path);
            return Err(error);
        }
    };
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
        doc.init_project_with_options(false, commit, false)?;
        Ok(())
    })();

    if let Err(error) = result {
        let _ = env::set_current_dir(orig_path);
        let _ = fs::remove_dir_all(&target_path);
        return Err(OmniDocError::Project(format!(
            "Failed to create project: {error}"
        )));
    }
    let _ = env::set_current_dir(orig_path);

    Ok(new_project_report(
        target_path,
        title,
        author,
        template,
        false,
        commit,
        true,
        actions,
    ))
}

#[allow(clippy::too_many_arguments)]
fn new_project_report(
    project_root: PathBuf,
    title: String,
    author: String,
    template: ProjectTemplateInfo,
    dry_run: bool,
    commit: bool,
    applied: bool,
    actions: Vec<ProjectUpdateAction>,
) -> NewProjectReport {
    NewProjectReport {
        schema_version: 1,
        project_root: project_root.to_string_lossy().to_string(),
        title,
        author,
        template,
        dry_run,
        commit,
        applied,
        actions,
    }
}

fn print_new_project_report(report: &NewProjectReport) {
    let project_root = Path::new(&report.project_root);
    if report.dry_run {
        println!("Project creation plan for {}:", project_root.display());
        println!("  Title:    {}", report.title);
        println!("  Author:   {}", report.author);
        println!(
            "  Template: {} ({})",
            report.template.key,
            report.template.format.as_str()
        );
        println!("  Actions:");
        for action in &report.actions {
            print_new_action(project_root, action);
        }
        println!("No files were changed.");
        return;
    }

    println!(
        "✓ Ready: '{}' ({}, {})",
        project_root.display(),
        report.template.key,
        report.template.format.as_str()
    );
    println!("  Next: omnidoc build {:?}", project_root);
    if !report.commit {
        println!("  Git changes were left uncommitted (--no-commit).");
    }
}

fn print_new_action(project_root: &Path, action: &ProjectUpdateAction) {
    let path = Path::new(&action.path);
    let display = if path == project_root {
        path.display().to_string()
    } else {
        path.strip_prefix(project_root)
            .unwrap_or(path)
            .display()
            .to_string()
    };
    match action.operation.as_str() {
        "create_directory" => println!("    Would create directory {display}"),
        "create_file" => println!("    Would create file {display}"),
        "initialize_git" => println!("    Would initialize Git at {display}"),
        "commit" => println!("    Would create the initial commit"),
        operation => println!("    Would {operation} {display}"),
    }
}
