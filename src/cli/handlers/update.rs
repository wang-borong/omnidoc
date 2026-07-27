use crate::build::pipeline::{detect_project_type, ProjectType};
use crate::cli::handlers::common::{
    create_config_manager_default, dirty_auto_commit_error, format_git_change,
    merged_config_to_envs, print_json_error, user_git_changes,
};
use crate::doc::{Doc, ProjectUpdateAction};
use crate::error::{OmniDocError, Result};
use crate::git::{git_has_commits, is_git_repo, GitWorktreeChange};
use crate::utils::path;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize)]
struct UpdateRepositoryStatus {
    exists: bool,
    has_commits: bool,
    clean: bool,
    changes: Vec<GitWorktreeChange>,
}

#[derive(Debug, Serialize)]
struct UpdateReport {
    schema_version: u32,
    project_root: String,
    dry_run: bool,
    diff: bool,
    commit: bool,
    will_commit: bool,
    ready: bool,
    applied: bool,
    repository: UpdateRepositoryStatus,
    actions: Vec<ProjectUpdateAction>,
}

/// Handle the 'update' command
pub fn handle_update(
    path: Option<String>,
    dry_run: bool,
    diff: bool,
    no_commit: bool,
    json: bool,
) -> Result<()> {
    let report = match update_project(path, dry_run || diff, diff, !no_commit) {
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

fn update_project(
    path: Option<String>,
    dry_run: bool,
    include_diff: bool,
    commit: bool,
) -> Result<UpdateReport> {
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
    let mut actions = doc.plan_update(commit, include_diff)?;
    let mut repository = resolve_repository_status(&project_path)?;
    let mut will_commit = commit && actions.iter().any(|action| action.operation == "commit");
    let mut ready =
        !will_commit || !repository.exists || !repository.has_commits || repository.clean;

    if !dry_run {
        let _project_lock =
            crate::project_tools::acquire_project_write_lock(&project_path, "update project")?;
        actions = doc.plan_update(commit, false)?;
        repository = resolve_repository_status(&project_path)?;
        will_commit = commit && actions.iter().any(|action| action.operation == "commit");
        ready = !will_commit || !repository.exists || !repository.has_commits || repository.clean;
        if !ready {
            return Err(dirty_auto_commit_error(
                "an update commit",
                "--no-commit",
                &repository.changes,
            ));
        }
        doc.update_project_with_options(will_commit, false)?;
    }

    Ok(UpdateReport {
        schema_version: 1,
        project_root: path_str,
        dry_run,
        diff: include_diff,
        commit,
        will_commit,
        ready,
        applied: !dry_run,
        repository,
        actions,
    })
}

fn resolve_repository_status(project_path: &Path) -> Result<UpdateRepositoryStatus> {
    if !is_git_repo(project_path) {
        return Ok(UpdateRepositoryStatus {
            exists: false,
            has_commits: false,
            clean: true,
            changes: Vec::new(),
        });
    }

    let has_commits = git_has_commits(project_path)?;
    let changes = user_git_changes(project_path)?;
    Ok(UpdateRepositoryStatus {
        exists: true,
        has_commits,
        clean: changes.is_empty(),
        changes,
    })
}

fn print_update_report(report: &UpdateReport) {
    let project_root = Path::new(&report.project_root);
    if report.dry_run {
        println!("Update plan for {}:", project_root.display());
    } else {
        println!("Updated {}:", project_root.display());
    }

    if !report.repository.clean {
        let message = if report.will_commit && !report.repository.has_commits {
            "Repository has no commits; current project files will be included in the first update commit."
        } else if !report.commit {
            "Repository already has changes; no automatic commit was requested."
        } else if !report.will_commit {
            "No update commit is needed; existing repository changes will not be staged."
        } else {
            "Automatic commit is blocked until these repository changes are committed or stashed:"
        };
        println!("{message}");
        for change in &report.repository.changes {
            println!("  {}", format_git_change(change));
        }
    }

    for action in &report.actions {
        let path = display_path(project_root, &action.path);
        match (
            report.dry_run,
            action.operation.as_str(),
            action.change.as_deref(),
            action.destination.as_deref(),
        ) {
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
            (true, "initialize_git", _, _) => {
                println!("  Would initialize Git repository at {path}")
            }
            (false, "initialize_git", _, _) => {
                println!("  Initialized Git repository at {path}")
            }
            (true, "commit", _, _) => println!("  Would create an update commit"),
            (false, "commit", _, _) => println!("  Created an update commit"),
            (true, operation, _, _) => println!("  Would {operation} {path}"),
            (false, operation, _, _) => println!("  Completed {operation} {path}"),
        }
        if let Some(diff) = &action.diff {
            print!("{}", diff);
            if !diff.ends_with('\n') {
                println!();
            }
        }
    }

    if report.dry_run {
        println!("No files were changed.");
    } else if !report.commit {
        println!("Changes were left uncommitted (--no-commit).");
    } else if report.actions.is_empty() {
        println!("Project scaffolding is already up to date.");
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
