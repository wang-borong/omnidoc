use crate::cli::handlers::common::{
    check_omnidoc_project, create_config_manager_default, print_json_error,
};
use crate::doc::artifacts::{
    artifact_for_format, configured_output_formats, entry_path, output_directory,
    primary_output_format, target_name, ProjectArtifact,
};
use crate::error::{OmniDocError, Result};
use crate::utils::path;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
struct PathStatus {
    path: String,
    exists: bool,
}

impl PathStatus {
    fn new(path: PathBuf) -> Self {
        Self {
            exists: path.exists(),
            path: path.to_string_lossy().to_string(),
        }
    }
}

#[derive(Debug, Serialize)]
struct ProjectStatus {
    schema_version: u32,
    project_root: String,
    config_file: PathStatus,
    entry: PathStatus,
    source_format: String,
    target: String,
    output_directory: PathStatus,
    default_output: String,
    configured_outputs: Vec<String>,
    artifacts: Vec<ProjectArtifact>,
}

/// Show the resolved project configuration and expected build artifacts.
pub fn handle_status(path: Option<String>, json: bool) -> Result<()> {
    let status = match resolve_status(path) {
        Ok(status) => status,
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
            serde_json::to_string_pretty(&status).map_err(|error| {
                OmniDocError::Other(format!("Failed to serialize project status: {error}"))
            })?
        );
    } else {
        let project_path = Path::new(&status.project_root);
        print_human_status(project_path, &status);
    }

    Ok(())
}

fn resolve_status(path: Option<String>) -> Result<ProjectStatus> {
    let project_path = path::determine_project_root(path)?;
    check_omnidoc_project(&project_path)?;

    let config_manager = create_config_manager_default(Some(&project_path))?;
    let config = config_manager.get_merged();
    let entry = entry_path(&project_path, config);
    let outputs = configured_output_formats(config)?;
    let artifacts = outputs
        .iter()
        .map(|output| artifact_for_format(&project_path, config, output))
        .collect::<Result<Vec<_>>>()?;
    let source_format = config.from.clone().unwrap_or_else(|| {
        match entry.extension().and_then(|extension| extension.to_str()) {
            Some("tex") => "latex".to_string(),
            _ => "markdown".to_string(),
        }
    });
    Ok(ProjectStatus {
        schema_version: 1,
        project_root: project_path.to_string_lossy().to_string(),
        config_file: PathStatus::new(project_path.join(".omnidoc.toml")),
        entry: PathStatus::new(entry),
        source_format,
        target: target_name(&project_path, config),
        output_directory: PathStatus::new(output_directory(&project_path, config)),
        default_output: primary_output_format(config)?,
        configured_outputs: outputs,
        artifacts,
    })
}

fn print_human_status(project_path: &Path, status: &ProjectStatus) {
    println!("Project: {}", status.project_root);
    println!(
        "Config:  {} ({})",
        display_path(project_path, &status.config_file.path),
        readiness(status.config_file.exists)
    );
    println!(
        "Entry:   {} ({}, {})",
        display_path(project_path, &status.entry.path),
        status.source_format,
        readiness(status.entry.exists)
    );
    println!("Target:  {}", status.target);
    println!(
        "Output:  {} (default: {})",
        display_path(project_path, &status.output_directory.path),
        status.default_output
    );
    println!("Artifacts:");
    for artifact in &status.artifacts {
        let detail = artifact
            .bytes
            .map(format_bytes)
            .unwrap_or_else(|| "missing".to_string());
        println!(
            "  {:<6} {:<8} {}",
            artifact.format,
            detail,
            display_path(project_path, &artifact.path)
        );
    }
}

fn display_path(project_path: &Path, path: &str) -> String {
    let path = Path::new(path);
    match path.strip_prefix(project_path) {
        Ok(relative) if relative.as_os_str().is_empty() => ".".to_string(),
        Ok(relative) => relative.display().to_string(),
        Err(_) => path.display().to_string(),
    }
}

fn readiness(exists: bool) -> &'static str {
    if exists {
        "ready"
    } else {
        "missing"
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

#[cfg(test)]
mod tests {
    use super::{display_path, format_bytes};
    use std::path::Path;

    #[test]
    fn status_formats_relative_paths_and_sizes_for_humans() {
        let root = Path::new("/tmp/project");
        assert_eq!(
            display_path(root, "/tmp/project/build/book.pdf"),
            "build/book.pdf"
        );
        assert_eq!(format_bytes(100), "100 B");
        assert_eq!(format_bytes(2048), "2.0 KiB");
    }
}
