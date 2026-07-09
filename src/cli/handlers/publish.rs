use crate::cli::handlers::build::{
    build_cli_overrides, build_project_outputs, expected_output_file, resolve_outputs,
    BuildRunOptions,
};
use crate::cli::handlers::common::create_config_manager;
use crate::error::{OmniDocError, Result};
use crate::utils::path;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize)]
struct PublishArtifact {
    output: String,
    source: String,
    destination: String,
    bytes: u64,
}

#[derive(Debug, Serialize)]
struct PublishManifest {
    omnidoc_version: String,
    project: String,
    target: String,
    tag: String,
    published_at_unix: u64,
    artifacts: Vec<PublishArtifact>,
}

#[allow(clippy::too_many_arguments)]
pub fn handle_publish(
    path: Option<String>,
    to: Option<String>,
    all: bool,
    outputs: Vec<String>,
    pdf_engine: Option<String>,
    latex_backend: String,
    max_latex_passes: Option<usize>,
    dist_dir: String,
    tag: Option<String>,
    no_build: bool,
    force: bool,
    strict: bool,
    verbose: bool,
) -> Result<()> {
    let project_path = path::determine_project_path(path)?.canonicalize()?;
    let cli_overrides = build_cli_overrides(
        to,
        outputs,
        pdf_engine,
        latex_backend,
        max_latex_passes,
        verbose,
    );

    if !no_build {
        build_project_outputs(
            &project_path,
            cli_overrides.clone(),
            all,
            BuildRunOptions {
                force,
                report: true,
                write_lock: true,
                strict,
            },
            verbose,
        )?;
    }

    let config_manager = create_config_manager(Some(&project_path), cli_overrides.clone())?;
    let config = config_manager.get_merged().clone();
    let outputs = resolve_outputs(&config, &cli_overrides, all);
    let target = config.target.clone().unwrap_or_else(|| {
        project_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("document")
            .to_string()
    });
    let tag = tag.unwrap_or_else(|| target.clone());
    let publish_dir = resolve_publish_dir(&project_path, &dist_dir).join(sanitize_path_part(&tag));
    fs::create_dir_all(&publish_dir)?;

    let mut artifacts = Vec::new();
    for output in outputs {
        let source = expected_output_file(&project_path, &config, &output, &target);
        artifacts.push(copy_artifact(&source, &publish_dir, &output)?);
    }

    if let Some(lock_artifact) =
        copy_optional_sidecar(&project_path.join("omnidoc.lock"), &publish_dir, "lock")?
    {
        artifacts.push(lock_artifact);
    }

    let report_path = config
        .outdir
        .as_ref()
        .map(|outdir| project_path.join(outdir))
        .unwrap_or_else(|| project_path.join("build"))
        .join("omnidoc-report.json");
    if let Some(report_artifact) = copy_optional_sidecar(&report_path, &publish_dir, "report")? {
        artifacts.push(report_artifact);
    }

    let manifest = PublishManifest {
        omnidoc_version: env!("CARGO_PKG_VERSION").to_string(),
        project: project_path.display().to_string(),
        target,
        tag,
        published_at_unix: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0),
        artifacts,
    };
    let manifest_content = serde_json::to_string_pretty(&manifest)
        .map_err(|err| OmniDocError::Other(err.to_string()))?;
    fs::write(publish_dir.join("omnidoc-publish.json"), manifest_content)?;

    println!("Published artifacts to {}", publish_dir.display());
    Ok(())
}

fn copy_optional_sidecar(
    source: &Path,
    publish_dir: &Path,
    output: &str,
) -> Result<Option<PublishArtifact>> {
    if !source.exists() {
        return Ok(None);
    }

    copy_artifact(source, publish_dir, output).map(Some)
}

fn copy_artifact(source: &Path, publish_dir: &Path, output: &str) -> Result<PublishArtifact> {
    if !source.exists() {
        return Err(OmniDocError::Project(format!(
            "Publish artifact not found: {}",
            source.display()
        )));
    }
    let file_name = source.file_name().ok_or_else(|| {
        OmniDocError::Project(format!(
            "Publish artifact has no file name: {}",
            source.display()
        ))
    })?;
    let destination = publish_dir.join(file_name);
    fs::copy(source, &destination)?;
    let bytes = fs::metadata(&destination)?.len();

    Ok(PublishArtifact {
        output: output.to_string(),
        source: source.display().to_string(),
        destination: destination.display().to_string(),
        bytes,
    })
}

fn resolve_publish_dir(project_path: &Path, dist_dir: &str) -> PathBuf {
    let path = Path::new(dist_dir);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        project_path.join(path)
    }
}

fn sanitize_path_part(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | ':' => '-',
            _ => ch,
        })
        .collect::<String>();
    if sanitized.trim().is_empty() {
        "release".to_string()
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use super::sanitize_path_part;

    #[test]
    fn sanitizes_publish_tag() {
        assert_eq!(sanitize_path_part("v1/report"), "v1-report");
        assert_eq!(sanitize_path_part(""), "release");
    }
}
