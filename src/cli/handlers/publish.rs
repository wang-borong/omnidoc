use crate::cli::handlers::build::{
    build_cli_overrides, build_project_outputs, expected_output_file, resolve_outputs,
    BuildRunOptions,
};
use crate::cli::handlers::common::create_config_manager;
use crate::error::{OmniDocError, Result};
use crate::project_tools::content_digest;
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
    digest: String,
}

#[derive(Debug, Serialize)]
struct PublishManifest {
    manifest_version: u32,
    omnidoc_version: String,
    project: String,
    target: String,
    tag: String,
    published_at_unix: u64,
    library_contract: toml::Value,
    artifacts: Vec<PublishArtifact>,
}

const LIBRARY_RELEASE_CONTRACT: &str = include_str!("../../../release/omnidoc-libs.toml");

struct PublishTransaction {
    final_dir: PathBuf,
    staging_dir: PathBuf,
    backup_dir: PathBuf,
    committed: bool,
}

impl PublishTransaction {
    fn new(final_dir: PathBuf) -> Result<Self> {
        let parent = final_dir.parent().ok_or_else(|| {
            OmniDocError::Other(format!(
                "publish directory has no parent: {}",
                final_dir.display()
            ))
        })?;
        fs::create_dir_all(parent)?;
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let name = final_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("release");
        let suffix = format!("{}.{nonce}", std::process::id());
        let staging_dir = final_dir.with_file_name(format!(".{name}.staging.{suffix}"));
        let backup_dir = final_dir.with_file_name(format!(".{name}.backup.{suffix}"));
        fs::create_dir(&staging_dir)?;
        Ok(Self {
            final_dir,
            staging_dir,
            backup_dir,
            committed: false,
        })
    }

    fn path(&self) -> &Path {
        &self.staging_dir
    }

    fn commit(mut self) -> Result<PathBuf> {
        let had_existing = self.final_dir.exists();
        if had_existing {
            fs::rename(&self.final_dir, &self.backup_dir)?;
        }
        if let Err(error) = fs::rename(&self.staging_dir, &self.final_dir) {
            if had_existing {
                if let Err(restore_error) = fs::rename(&self.backup_dir, &self.final_dir) {
                    return Err(OmniDocError::Other(format!(
                        "failed to publish release ({error}); failed to restore previous release ({restore_error})"
                    )));
                }
            }
            return Err(error.into());
        }
        self.committed = true;
        if had_existing {
            if let Err(error) = remove_path(&self.backup_dir) {
                eprintln!(
                    "Warning: release published but old backup could not be removed: {} ({})",
                    self.backup_dir.display(),
                    error
                );
            }
        }
        Ok(self.final_dir.clone())
    }
}

impl Drop for PublishTransaction {
    fn drop(&mut self) {
        if !self.committed && self.staging_dir.exists() {
            let _ = remove_path(&self.staging_dir);
        }
    }
}

fn remove_path(path: &Path) -> std::io::Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
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
    let final_publish_dir =
        resolve_publish_dir(&project_path, &dist_dir).join(sanitize_path_part(&tag));
    let transaction = PublishTransaction::new(final_publish_dir)?;
    let publish_dir = transaction.path();

    let mut artifacts = Vec::new();
    for output in outputs {
        let source = expected_output_file(&project_path, &config, &output, &target);
        artifacts.push(copy_artifact(&project_path, &source, publish_dir, &output)?);
    }

    if let Some(lock_artifact) = copy_optional_sidecar(
        &project_path,
        &project_path.join("omnidoc.lock"),
        publish_dir,
        "lock",
    )? {
        artifacts.push(lock_artifact);
    }

    let report_path = config
        .outdir
        .as_ref()
        .map(|outdir| project_path.join(outdir))
        .unwrap_or_else(|| project_path.join("build"))
        .join("omnidoc-report.json");
    if let Some(report_artifact) =
        copy_optional_sidecar(&project_path, &report_path, publish_dir, "report")?
    {
        artifacts.push(report_artifact);
    }

    artifacts.push(write_embedded_artifact(
        LIBRARY_RELEASE_CONTRACT,
        publish_dir,
        "omnidoc-libs.toml",
        "library-contract",
    )?);

    let library_contract = toml::from_str(LIBRARY_RELEASE_CONTRACT).map_err(|error| {
        OmniDocError::Other(format!("invalid embedded library contract: {error}"))
    })?;

    let manifest = PublishManifest {
        manifest_version: 2,
        omnidoc_version: env!("CARGO_PKG_VERSION").to_string(),
        project: project_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("project")
            .to_string(),
        target,
        tag,
        published_at_unix: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0),
        library_contract,
        artifacts,
    };
    let manifest_content = serde_json::to_string_pretty(&manifest)
        .map_err(|err| OmniDocError::Other(err.to_string()))?;
    fs::write(publish_dir.join("omnidoc-publish.json"), manifest_content)?;

    let publish_dir = transaction.commit()?;
    println!("Published artifacts to {}", publish_dir.display());
    Ok(())
}

fn copy_optional_sidecar(
    project_path: &Path,
    source: &Path,
    publish_dir: &Path,
    output: &str,
) -> Result<Option<PublishArtifact>> {
    if !source.exists() {
        return Ok(None);
    }

    copy_artifact(project_path, source, publish_dir, output).map(Some)
}

fn copy_artifact(
    project_path: &Path,
    source: &Path,
    publish_dir: &Path,
    output: &str,
) -> Result<PublishArtifact> {
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
    let source_label = source
        .strip_prefix(project_path)
        .unwrap_or_else(|_| source.file_name().map(Path::new).unwrap_or(source))
        .to_string_lossy()
        .replace('\\', "/");

    Ok(PublishArtifact {
        output: output.to_string(),
        source: source_label,
        destination: file_name.to_string_lossy().to_string(),
        bytes,
        digest: content_digest(&destination)?,
    })
}

fn write_embedded_artifact(
    content: &str,
    publish_dir: &Path,
    file_name: &str,
    output: &str,
) -> Result<PublishArtifact> {
    let destination = publish_dir.join(file_name);
    fs::write(&destination, content)?;
    Ok(PublishArtifact {
        output: output.to_string(),
        source: format!("embedded:release/{file_name}"),
        destination: file_name.to_string(),
        bytes: fs::metadata(&destination)?.len(),
        digest: content_digest(&destination)?,
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
