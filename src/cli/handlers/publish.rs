use crate::cli::handlers::build::{
    build_cli_overrides, build_project_outputs_unlocked, expected_output_file, resolve_outputs,
    BuildRunOptions,
};
use crate::cli::handlers::common::create_config_manager;
use crate::error::{OmniDocError, Result};
use crate::project_tools::content_digest;
use crate::utils::path;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Deserialize, Serialize)]
struct PublishArtifact {
    output: String,
    source: String,
    destination: String,
    bytes: u64,
    digest: String,
}

#[derive(Debug, Deserialize, Serialize)]
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

#[derive(Debug, Serialize)]
struct PublishVerification {
    path: String,
    valid: bool,
    checked_artifacts: usize,
    errors: Vec<String>,
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
                crate::terminal::warning(format!(
                    "Release published, but its old backup could not be removed\n{} ({})",
                    self.backup_dir.display(),
                    error
                ));
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
    verify: bool,
    json: bool,
    force: bool,
    strict: bool,
    verbose: bool,
) -> Result<()> {
    let project_path = path::determine_project_path(path)?.canonicalize()?;
    if verify {
        let tag = tag.expect("clap requires --tag with --verify");
        return verify_published_release(&project_path, &dist_dir, &tag, json);
    }
    let _project_lock =
        crate::project_tools::acquire_project_write_lock(&project_path, "publish project")?;
    let cli_overrides = build_cli_overrides(
        to,
        outputs,
        pdf_engine,
        latex_backend,
        max_latex_passes,
        verbose,
    );

    if !no_build {
        build_project_outputs_unlocked(
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

fn verify_published_release(
    project_path: &Path,
    dist_dir: &str,
    tag: &str,
    json: bool,
) -> Result<()> {
    let publish_dir = resolve_publish_dir(project_path, dist_dir).join(sanitize_path_part(tag));
    let mut verification = PublishVerification {
        path: publish_dir.display().to_string(),
        valid: false,
        checked_artifacts: 0,
        errors: Vec::new(),
    };
    let manifest_path = publish_dir.join("omnidoc-publish.json");
    let manifest = match fs::read_to_string(&manifest_path) {
        Ok(content) => match serde_json::from_str::<PublishManifest>(&content) {
            Ok(manifest) => manifest,
            Err(error) => {
                verification
                    .errors
                    .push(format!("invalid publish manifest: {error}"));
                return finish_verification(verification, json);
            }
        },
        Err(error) => {
            verification.errors.push(format!(
                "cannot read publish manifest {}: {}",
                manifest_path.display(),
                error
            ));
            return finish_verification(verification, json);
        }
    };
    if manifest.manifest_version != 2 {
        verification.errors.push(format!(
            "unsupported publish manifest version {}",
            manifest.manifest_version
        ));
    }
    if manifest.tag != tag {
        verification.errors.push(format!(
            "publish tag '{}' does not match requested '{}'",
            manifest.tag, tag
        ));
    }
    let expected_library_contract = match toml::from_str::<toml::Value>(LIBRARY_RELEASE_CONTRACT) {
        Ok(expected) => {
            if manifest.library_contract != expected {
                verification.errors.push(
                    "embedded library contract does not match this OmniDoc release".to_string(),
                );
            }
            Some(expected)
        }
        Err(error) => {
            verification
                .errors
                .push(format!("invalid built-in library contract: {error}"));
            None
        }
    };
    if let Some(expected) = expected_library_contract {
        match fs::read_to_string(publish_dir.join("omnidoc-libs.toml"))
            .ok()
            .and_then(|content| toml::from_str::<toml::Value>(&content).ok())
        {
            Some(sidecar) if sidecar == expected => {}
            _ => verification.errors.push(
                "published omnidoc-libs.toml does not match this OmniDoc release".to_string(),
            ),
        }
    }

    let mut expected_files = BTreeSet::from(["omnidoc-publish.json".to_string()]);
    for artifact in &manifest.artifacts {
        if !safe_publish_file_name(&artifact.destination) {
            verification.errors.push(format!(
                "unsafe publish artifact destination: {}",
                artifact.destination
            ));
            continue;
        }
        if !artifact.source.starts_with("embedded:") && !safe_publish_source(&artifact.source) {
            verification.errors.push(format!(
                "non-portable publish artifact source: {}",
                artifact.source
            ));
        }
        if !expected_files.insert(artifact.destination.clone()) {
            verification.errors.push(format!(
                "duplicate publish artifact destination: {}",
                artifact.destination
            ));
            continue;
        }
        let path = publish_dir.join(&artifact.destination);
        let metadata = match path.symlink_metadata() {
            Ok(metadata) if metadata.file_type().is_file() => metadata,
            Ok(_) => {
                verification.errors.push(format!(
                    "publish artifact is not a regular file: {}",
                    artifact.destination
                ));
                continue;
            }
            Err(error) => {
                verification.errors.push(format!(
                    "missing publish artifact {}: {}",
                    artifact.destination, error
                ));
                continue;
            }
        };
        if metadata.len() != artifact.bytes {
            verification.errors.push(format!(
                "publish artifact size mismatch: {}",
                artifact.destination
            ));
        }
        match content_digest(&path) {
            Ok(digest) if digest != artifact.digest => verification.errors.push(format!(
                "publish artifact digest mismatch: {}",
                artifact.destination
            )),
            Err(error) => verification.errors.push(format!(
                "cannot hash publish artifact {}: {}",
                artifact.destination, error
            )),
            _ => {}
        }
        verification.checked_artifacts += 1;
    }

    match fs::read_dir(&publish_dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !expected_files.contains(&name) {
                    verification
                        .errors
                        .push(format!("unexpected file in published release: {name}"));
                }
            }
        }
        Err(error) => verification.errors.push(format!(
            "cannot inspect publish directory {}: {}",
            publish_dir.display(),
            error
        )),
    }
    verification.valid = verification.errors.is_empty();
    finish_verification(verification, json)
}

fn safe_publish_file_name(value: &str) -> bool {
    !value.is_empty()
        && !value.chars().any(|ch| matches!(ch, '/' | '\\' | ':'))
        && Path::new(value)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
        && Path::new(value).components().count() == 1
}

fn safe_publish_source(value: &str) -> bool {
    !value.is_empty()
        && !value.chars().any(|ch| matches!(ch, '\\' | ':'))
        && !Path::new(value).is_absolute()
        && Path::new(value)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn finish_verification(verification: PublishVerification, json: bool) -> Result<()> {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&verification)
                .map_err(|error| OmniDocError::Other(error.to_string()))?
        );
    } else {
        println!("path: {}", verification.path);
        println!("valid: {}", verification.valid);
        println!("checked artifacts: {}", verification.checked_artifacts);
        for error in &verification.errors {
            println!("error: {error}");
        }
    }
    if verification.valid {
        Ok(())
    } else {
        Err(OmniDocError::Other(
            "published release verification failed".to_string(),
        ))
    }
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
