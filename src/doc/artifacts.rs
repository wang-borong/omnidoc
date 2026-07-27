use crate::build::pandoc_policy::PandocOutputKind;
use crate::config::MergedConfig;
use crate::error::Result;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProjectArtifact {
    pub format: String,
    pub path: String,
    pub exists: bool,
    pub bytes: Option<u64>,
}

impl ProjectArtifact {
    pub fn from_path(format: String, path: PathBuf) -> Self {
        let metadata = path.metadata().ok().filter(|metadata| metadata.is_file());
        Self {
            format,
            path: path.to_string_lossy().to_string(),
            exists: metadata.is_some(),
            bytes: metadata.map(|metadata| metadata.len()),
        }
    }

    pub fn path_buf(&self) -> PathBuf {
        PathBuf::from(&self.path)
    }
}

pub fn canonical_output_format(output: &str) -> Result<String> {
    PandocOutputKind::from_requested(Some(output)).map(|kind| kind.config_key().to_string())
}

pub fn primary_output_format(config: &MergedConfig) -> Result<String> {
    canonical_output_format(config.to.as_deref().unwrap_or("pdf"))
}

pub fn configured_output_formats(config: &MergedConfig) -> Result<Vec<String>> {
    let mut outputs = vec![primary_output_format(config)?];
    for output in &config.outputs {
        let output = canonical_output_format(output)?;
        if !outputs.contains(&output) {
            outputs.push(output);
        }
    }
    Ok(outputs)
}

pub fn output_directory(project_path: &Path, config: &MergedConfig) -> PathBuf {
    config
        .outdir
        .as_ref()
        .map(|outdir| project_path.join(outdir))
        .unwrap_or_else(|| project_path.join("build"))
}

pub fn target_name(project_path: &Path, config: &MergedConfig) -> String {
    config.target.clone().unwrap_or_else(|| {
        project_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("document")
            .to_string()
    })
}

pub fn entry_path(project_path: &Path, config: &MergedConfig) -> PathBuf {
    if let Some(entry) = &config.entry {
        return project_path.join(entry);
    }

    match config
        .from
        .as_deref()
        .map(|format| format.to_ascii_lowercase())
    {
        Some(format) if matches!(format.as_str(), "latex" | "tex") => project_path.join("main.tex"),
        Some(format) if matches!(format.as_str(), "markdown" | "md") => {
            project_path.join("main.md")
        }
        _ if project_path.join("main.md").is_file() => project_path.join("main.md"),
        _ => project_path.join("main.tex"),
    }
}

pub fn expected_output_file(
    project_path: &Path,
    config: &MergedConfig,
    output: &str,
    target: &str,
) -> PathBuf {
    let extension = PandocOutputKind::from_requested(Some(output))
        .map(|kind| kind.extension().to_string())
        .unwrap_or_else(|_| output.trim().to_ascii_lowercase());
    output_directory(project_path, config).join(format!("{target}.{extension}"))
}

pub fn artifact_for_format(
    project_path: &Path,
    config: &MergedConfig,
    output: &str,
) -> Result<ProjectArtifact> {
    let format = canonical_output_format(output)?;
    let target = target_name(project_path, config);
    let path = expected_output_file(project_path, config, &format, &target);
    Ok(ProjectArtifact::from_path(format, path))
}

#[cfg(test)]
mod tests {
    use super::{
        artifact_for_format, configured_output_formats, entry_path, expected_output_file,
        output_directory, target_name,
    };
    use crate::config::MergedConfig;
    use std::path::Path;

    #[test]
    fn resolves_configured_artifacts_from_project_contract() {
        let project = Path::new("/tmp/guide");
        let config = MergedConfig {
            entry: Some("docs/index.md".to_string()),
            from: Some("markdown".to_string()),
            to: Some("html5".to_string()),
            outputs: vec!["HTML".to_string(), "latex".to_string()],
            target: Some("handbook".to_string()),
            outdir: Some("output".to_string()),
            ..Default::default()
        };

        assert_eq!(target_name(project, &config), "handbook");
        assert_eq!(entry_path(project, &config), project.join("docs/index.md"));
        assert_eq!(output_directory(project, &config), project.join("output"));
        assert_eq!(
            configured_output_formats(&config).expect("formats"),
            vec!["html", "latex"]
        );
        assert_eq!(
            expected_output_file(project, &config, "latex", "handbook"),
            project.join("output/handbook.tex")
        );

        let artifact = artifact_for_format(project, &config, "html5").expect("artifact");
        assert_eq!(artifact.format, "html");
        assert_eq!(artifact.path_buf(), project.join("output/handbook.html"));
        assert!(!artifact.exists);
    }
}
