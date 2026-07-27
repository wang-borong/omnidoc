use crate::cli::handlers::common::{
    create_config_manager_default, dirty_auto_commit_error, format_git_change,
    merged_config_to_envs, print_json_error, resolve_repository_status, GitRepositoryStatus,
};
use crate::cli::utils::{
    infer_title, require_explicit_creation_template, resolve_creation_template,
};
use crate::config::ProjectConfig;
use crate::constants::paths;
use crate::doc::templates::ProjectTemplateInfo;
use crate::doc::{Doc, ProjectUpdateAction};
use crate::doctype::DocumentFormat;
use crate::error::{OmniDocError, Result};
use crate::utils::fs;
use serde::Serialize;
use std::env;
use std::path::Path;

#[derive(Debug, Serialize)]
struct InitProjectReport {
    schema_version: u32,
    project_root: String,
    title: String,
    author: String,
    template: ProjectTemplateInfo,
    dry_run: bool,
    diff: bool,
    commit: bool,
    will_commit: bool,
    ready: bool,
    applied: bool,
    repository: GitRepositoryStatus,
    actions: Vec<ProjectUpdateAction>,
}

/// Handle the `init` command.
#[allow(clippy::too_many_arguments)]
pub fn handle_init(
    title: Option<String>,
    author: Option<String>,
    doctype: Option<String>,
    format: Option<DocumentFormat>,
    defaults: bool,
    no_commit: bool,
    dry_run: bool,
    diff: bool,
    json: bool,
) -> Result<()> {
    if json {
        if let Err(error) = require_explicit_creation_template(doctype.as_deref(), defaults) {
            print_json_error(&error);
            return Err(error);
        }
    }
    let report = match initialize_existing_project(
        title,
        author,
        doctype,
        format,
        defaults,
        !no_commit,
        dry_run || diff,
        diff,
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
                    "Failed to serialize project initialization report: {error}"
                ))
            })?
        );
    } else {
        print_init_project_report(&report);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn initialize_existing_project(
    title: Option<String>,
    author: Option<String>,
    doctype: Option<String>,
    format: Option<DocumentFormat>,
    defaults: bool,
    commit: bool,
    dry_run: bool,
    include_diff: bool,
) -> Result<InitProjectReport> {
    let project_path = env::current_dir().map_err(OmniDocError::Io)?;
    if ProjectConfig::exists(&project_path) {
        return Err(OmniDocError::Project(
            "This is already an OmniDoc project, no action taken".to_string(),
        ));
    }

    let config_manager = create_config_manager_default(Some(&project_path))?;
    let merged_config = config_manager.get_merged();
    let envs = merged_config_to_envs(merged_config);
    let author = author
        .or_else(|| merged_config.author.clone())
        .unwrap_or_else(|| "Someone".to_string());
    let template = resolve_creation_template(doctype, format, defaults)?;
    let title = title.unwrap_or_else(|| infer_title(&project_path));
    let path_string = project_path.to_string_lossy().to_string();
    let doc = Doc::new(&title, &path_string, &author, &template.key, envs);
    let mut actions = doc.plan_init(commit, include_diff)?;
    let mut repository = resolve_repository_status(&project_path)?;
    let mut ready = !commit || !repository.exists || !repository.has_commits || repository.clean;

    if dry_run {
        return Ok(init_project_report(
            project_path,
            title,
            author,
            template,
            true,
            include_diff,
            commit,
            ready,
            false,
            repository,
            actions,
        ));
    }

    actions = doc.plan_init(commit, false)?;
    repository = resolve_repository_status(&project_path)?;
    ready = !commit || !repository.exists || !repository.has_commits || repository.clean;
    if !ready {
        return Err(dirty_auto_commit_error(
            "an initialization commit",
            "--no-commit",
            &repository.changes,
        ));
    }

    let target_name = project_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("document");
    ProjectConfig::create_default(
        &project_path,
        Some(&template.file_name),
        Some(template.format.as_str()),
        Some("pdf"),
        Some(target_name),
    )?;
    if let Err(error) = doc.init_project_with_options(false, commit, false) {
        let _ = fs::remove_file(project_path.join(paths::PROJECT_CONFIG));
        return Err(OmniDocError::Project(format!(
            "Failed to initialize project: {error}"
        )));
    }

    Ok(init_project_report(
        project_path,
        title,
        author,
        template,
        false,
        false,
        commit,
        ready,
        true,
        repository,
        actions,
    ))
}

#[allow(clippy::too_many_arguments)]
fn init_project_report(
    project_root: std::path::PathBuf,
    title: String,
    author: String,
    template: ProjectTemplateInfo,
    dry_run: bool,
    diff: bool,
    commit: bool,
    ready: bool,
    applied: bool,
    repository: GitRepositoryStatus,
    actions: Vec<ProjectUpdateAction>,
) -> InitProjectReport {
    InitProjectReport {
        schema_version: 1,
        project_root: project_root.to_string_lossy().to_string(),
        title,
        author,
        template,
        dry_run,
        diff,
        commit,
        will_commit: commit,
        ready,
        applied,
        repository,
        actions,
    }
}

fn print_init_project_report(report: &InitProjectReport) {
    let project_root = Path::new(&report.project_root);
    if report.dry_run {
        println!(
            "Project initialization plan for {}:",
            project_root.display()
        );
        println!("  Title:    {}", report.title);
        println!("  Author:   {}", report.author);
        println!(
            "  Template: {} ({})",
            report.template.key,
            report.template.format.as_str()
        );
    } else {
        println!("Initialized {}:", project_root.display());
    }

    print_repository_readiness(report);
    for action in &report.actions {
        print_init_action(project_root, report.dry_run, action);
    }

    if report.dry_run {
        println!("No files were changed.");
        return;
    }

    println!(
        "✓ Ready: '{}' ({}, {})",
        project_root.display(),
        report.template.key,
        report.template.format.as_str()
    );
    println!("  Next: omnidoc build");
    if !report.commit {
        println!("  Git changes were left uncommitted (--no-commit).");
    }
}

fn print_repository_readiness(report: &InitProjectReport) {
    if report.repository.clean {
        return;
    }
    let message = if !report.commit {
        "Repository already has changes; no automatic commit was requested."
    } else if !report.repository.has_commits {
        "Repository has no commits; existing files will be included in the first initialization commit."
    } else {
        "Automatic commit is blocked until these repository changes are committed or stashed:"
    };
    println!("{message}");
    for change in &report.repository.changes {
        println!("  {}", format_git_change(change));
    }
}

fn print_init_action(project_root: &Path, dry_run: bool, action: &ProjectUpdateAction) {
    let path = display_path(project_root, &action.path);
    match (
        dry_run,
        action.operation.as_str(),
        action.change.as_deref(),
        action.destination.as_deref(),
    ) {
        (true, "create_file", _, _) => println!("  Would create {path}"),
        (false, "create_file", _, _) => println!("  Created {path}"),
        (true, "refresh_file", Some("create"), _) => println!("  Would create {path}"),
        (true, "refresh_file", _, _) => println!("  Would update {path}"),
        (false, "refresh_file", Some("create"), _) => println!("  Created {path}"),
        (false, "refresh_file", _, _) => println!("  Updated {path}"),
        (true, "create_directory", _, _) => println!("  Would create directory {path}"),
        (false, "create_directory", _, _) => println!("  Created directory {path}"),
        (true, "move_file", _, Some(destination)) => println!(
            "  Would move {path} -> {}",
            display_path(project_root, destination)
        ),
        (false, "move_file", _, Some(destination)) => println!(
            "  Moved {path} -> {}",
            display_path(project_root, destination)
        ),
        (true, "initialize_git", _, _) => println!("  Would initialize Git at {path}"),
        (false, "initialize_git", _, _) => println!("  Initialized Git at {path}"),
        (true, "commit", _, _) => println!("  Would create the initialization commit"),
        (false, "commit", _, _) => println!("  Created the initialization commit"),
        (true, operation, _, _) => println!("  Would {operation} {path}"),
        (false, operation, _, _) => println!("  Completed {operation} {path}"),
    }
    if let Some(diff) = &action.diff {
        print!("{diff}");
        if !diff.ends_with('\n') {
            println!();
        }
    }
}

fn display_path(project_root: &Path, path: &str) -> String {
    let path = Path::new(path);
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .display()
        .to_string()
}
