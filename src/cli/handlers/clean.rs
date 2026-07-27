use crate::cli::handlers::common::{check_omnidoc_project, create_build_service, print_json_error};
use crate::config::CliOverrides;
use crate::doc::services::{CleanOptions, CleanReport, CleanTargetKind};
use crate::error::{OmniDocError, Result};
use crate::utils::path;
use std::path::Path;

/// Handle the 'clean' command
pub fn handle_clean(
    path: Option<String>,
    distclean: bool,
    dry_run: bool,
    json: bool,
) -> Result<()> {
    let report = match clean_project(path, distclean, dry_run) {
        Ok(report) => report,
        Err(error) => {
            if json {
                print_json_error(&error);
            }
            return Err(error);
        }
    };

    let project_path = Path::new(&report.project_root);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|error| {
                OmniDocError::Other(format!("Failed to serialize clean report: {error}"))
            })?
        );
    } else {
        print_clean_report(project_path, &report);
    }

    Ok(())
}

fn clean_project(path: Option<String>, distclean: bool, dry_run: bool) -> Result<CleanReport> {
    let project_path = path::determine_project_root(path)?;
    check_omnidoc_project(&project_path)?;
    let build_service = create_build_service(Some(&project_path), CliOverrides::new())?;
    let options = CleanOptions { distclean, dry_run };

    if dry_run {
        return build_service.clean_with_options(&project_path, options);
    }

    let _project_lock =
        crate::project_tools::acquire_project_write_lock(&project_path, "clean project")?;
    build_service.clean_with_options(&project_path, options)
}

fn print_clean_report(project_path: &Path, report: &CleanReport) {
    if report.targets.is_empty() {
        println!("Nothing to clean in {}.", project_path.display());
        return;
    }

    let verb = if report.dry_run {
        "Would remove"
    } else {
        "Removed"
    };
    for target in &report.targets {
        let path = Path::new(&target.path);
        let display = path.strip_prefix(project_path).unwrap_or(path).display();
        let kind = match target.kind {
            CleanTargetKind::File => "file",
            CleanTargetKind::Directory => "directory",
            CleanTargetKind::Symlink => "symlink",
        };
        println!(
            "{} {} {} ({} files, {})",
            verb,
            kind,
            display,
            target.files,
            format_bytes(target.bytes)
        );
    }
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    }
}
