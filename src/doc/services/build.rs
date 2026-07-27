use crate::build::pipeline::detect_project_type;
use crate::build::{BuildPipeline, LatexBuilder, PandocBuilder, ProjectType};
use crate::config::MergedConfig;
use crate::doc::artifacts::{
    configured_output_formats, entry_path, expected_output_file, target_name,
};
use crate::error::{OmniDocError, Result};
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Copy, Default)]
pub struct CleanOptions {
    pub distclean: bool,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CleanTargetKind {
    File,
    Directory,
    Symlink,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CleanTarget {
    pub path: String,
    pub kind: CleanTargetKind,
    pub files: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CleanReport {
    pub schema_version: u32,
    pub project_root: String,
    pub mode: String,
    pub dry_run: bool,
    pub targets: Vec<CleanTarget>,
    pub removed_targets: usize,
}

/// 构建服务
/// 统一管理构建流程，根据项目类型选择合适的构建器
pub struct BuildService {
    config: MergedConfig,
}

impl BuildService {
    pub fn new(config: MergedConfig) -> Self {
        Self { config }
    }

    /// 构建项目
    pub fn build(&self, project_path: &Path, verbose: bool) -> Result<()> {
        // 检测项目类型
        let project_type = self.detect_project_type(project_path)?;

        match project_type {
            ProjectType::Markdown => {
                let builder = PandocBuilder::new(self.config.clone())?;
                BuildPipeline::build(&builder, project_path, verbose)
            }
            ProjectType::Latex => {
                let builder = LatexBuilder::new(self.config.clone())?;
                BuildPipeline::build(&builder, project_path, verbose)
            }
            ProjectType::Unknown => {
                // 尝试使用 PandocBuilder（默认）
                let builder = PandocBuilder::new(self.config.clone())?;
                match BuildPipeline::build(&builder, project_path, verbose) {
                    Ok(_) => Ok(()),
                    Err(_) => {
                        // 如果 Pandoc 构建失败，尝试 LaTeX
                        let builder = LatexBuilder::new(self.config.clone())?;
                        BuildPipeline::build(&builder, project_path, verbose)
                    }
                }
            }
        }
    }

    /// 检测项目类型
    pub fn detect_project_type(&self, project_path: &Path) -> Result<ProjectType> {
        Ok(detect_project_type(&self.config, project_path))
    }

    /// 清理构建产物
    pub fn clean(&self, project_path: &Path, distclean: bool) -> Result<()> {
        self.clean_with_options(
            project_path,
            CleanOptions {
                distclean,
                dry_run: false,
            },
        )
        .map(|_| ())
    }

    pub fn clean_with_options(
        &self,
        project_path: &Path,
        options: CleanOptions,
    ) -> Result<CleanReport> {
        let project_root = project_path.canonicalize()?;
        let targets = self.clean_targets(&project_root, options.distclean)?;
        let removed_targets = if options.dry_run {
            0
        } else {
            for target in &targets {
                remove_clean_target(target)?;
            }
            targets.len()
        };

        Ok(CleanReport {
            schema_version: 1,
            project_root: project_root.to_string_lossy().to_string(),
            mode: if options.distclean {
                "distclean".to_string()
            } else {
                "clean".to_string()
            },
            dry_run: options.dry_run,
            targets,
            removed_targets,
        })
    }

    fn clean_targets(&self, project_root: &Path, distclean: bool) -> Result<Vec<CleanTarget>> {
        let output_dir = safe_output_directory(project_root, &self.config)?;
        let mut candidates = Vec::new();

        if output_dir == project_root {
            candidates.extend(self.root_output_candidates(project_root)?);
        } else {
            candidates.push(output_dir);
        }

        if distclean {
            candidates.extend(root_temporary_candidates(project_root, &self.config));
            candidates.push(project_root.join("auto"));
        }

        collect_clean_targets(project_root, candidates)
    }

    fn root_output_candidates(&self, project_root: &Path) -> Result<Vec<PathBuf>> {
        let target = target_name(project_root, &self.config);
        let mut candidates = configured_output_formats(&self.config)?
            .into_iter()
            .map(|output| expected_output_file(project_root, &self.config, &output, &target))
            .collect::<Vec<_>>();
        candidates.push(project_root.join("omnidoc-report.json"));
        candidates.extend(root_temporary_candidates(project_root, &self.config));
        Ok(candidates)
    }
}

fn safe_output_directory(project_root: &Path, config: &MergedConfig) -> Result<PathBuf> {
    let configured = config.outdir.as_deref().unwrap_or("build");
    let configured_path = Path::new(configured);
    if configured_path.is_absolute() {
        return Err(OmniDocError::Project(format!(
            "Refusing to clean absolute build.outdir '{}'. Use `omnidoc clean --dry-run` after choosing a project-relative output directory.",
            configured_path.display()
        )));
    }

    let mut output_dir = project_root.to_path_buf();
    for component in configured_path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(component) => output_dir.push(component),
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(OmniDocError::Project(format!(
                    "Refusing to clean build.outdir '{}' because it escapes the project root",
                    configured_path.display()
                )));
            }
        }
    }
    Ok(output_dir)
}

fn root_temporary_candidates(project_root: &Path, config: &MergedConfig) -> Vec<PathBuf> {
    let mut stems = BTreeSet::new();
    stems.insert(target_name(project_root, config));
    if let Some(stem) = entry_path(project_root, config)
        .file_stem()
        .and_then(|stem| stem.to_str())
    {
        stems.insert(stem.to_string());
    }

    let suffixes = [
        "aux",
        "bcf",
        "bbl",
        "blg",
        "fdb_latexmk",
        "fls",
        "log",
        "out",
        "run.xml",
        "synctex.gz",
        "toc",
        "xdv",
    ];
    stems
        .into_iter()
        .flat_map(|stem| {
            suffixes
                .iter()
                .map(move |suffix| project_root.join(format!("{stem}.{suffix}")))
        })
        .collect()
}

fn collect_clean_targets(
    project_root: &Path,
    candidates: Vec<PathBuf>,
) -> Result<Vec<CleanTarget>> {
    let mut unique = BTreeSet::new();
    let mut targets = Vec::new();
    for candidate in candidates {
        if !unique.insert(candidate.clone()) {
            continue;
        }
        let metadata = match std::fs::symlink_metadata(&candidate) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(OmniDocError::Io(error)),
        };

        let kind = if metadata.file_type().is_symlink() {
            CleanTargetKind::Symlink
        } else if metadata.is_dir() {
            let resolved = candidate.canonicalize()?;
            if resolved == project_root || !resolved.starts_with(project_root) {
                return Err(OmniDocError::Project(format!(
                    "Refusing to recursively clean unsafe directory '{}'",
                    candidate.display()
                )));
            }
            CleanTargetKind::Directory
        } else {
            CleanTargetKind::File
        };
        let (files, bytes) = summarize_target(&candidate, &kind)?;
        targets.push(CleanTarget {
            path: candidate.to_string_lossy().to_string(),
            kind,
            files,
            bytes,
        });
    }
    targets.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(targets)
}

fn summarize_target(path: &Path, kind: &CleanTargetKind) -> Result<(u64, u64)> {
    if !matches!(kind, CleanTargetKind::Directory) {
        let bytes = std::fs::symlink_metadata(path)
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        return Ok((1, bytes));
    }

    let mut files = 0;
    let mut bytes = 0;
    for entry in walkdir::WalkDir::new(path).min_depth(1) {
        let entry = entry?;
        if entry.file_type().is_file() || entry.file_type().is_symlink() {
            files += 1;
            bytes += entry.metadata().map(|metadata| metadata.len()).unwrap_or(0);
        }
    }
    Ok((files, bytes))
}

fn remove_clean_target(target: &CleanTarget) -> Result<()> {
    let path = Path::new(&target.path);
    match target.kind {
        CleanTargetKind::Directory => crate::utils::fs::remove_dir_all(path),
        CleanTargetKind::File | CleanTargetKind::Symlink => crate::utils::fs::remove_file(path),
    }
}

#[cfg(test)]
mod tests {
    use super::{BuildService, CleanOptions};
    use crate::config::MergedConfig;
    use std::fs;

    #[test]
    fn clean_preview_is_non_mutating_and_regular_clean_removes_output_directory() {
        let project = tempfile::tempdir().expect("project");
        fs::create_dir_all(project.path().join("build")).expect("build dir");
        fs::write(project.path().join("build/book.pdf"), "pdf").expect("artifact");
        fs::write(project.path().join("reference.pdf"), "source asset").expect("source pdf");
        let service = BuildService::new(MergedConfig {
            target: Some("book".to_string()),
            ..Default::default()
        });

        let preview = service
            .clean_with_options(
                project.path(),
                CleanOptions {
                    dry_run: true,
                    ..Default::default()
                },
            )
            .expect("preview");
        assert_eq!(preview.targets.len(), 1);
        assert_eq!(preview.targets[0].files, 1);
        assert!(project.path().join("build/book.pdf").is_file());

        service.clean(project.path(), false).expect("clean");
        assert!(!project.path().join("build").exists());
        assert!(project.path().join("reference.pdf").is_file());
    }

    #[test]
    fn distclean_only_removes_known_root_temporary_files() {
        let project = tempfile::tempdir().expect("project");
        fs::create_dir_all(project.path().join("auto")).expect("auto dir");
        fs::write(project.path().join("auto/generated.el"), "generated").expect("auto file");
        fs::write(project.path().join("book.aux"), "aux").expect("aux file");
        fs::write(project.path().join("reference.pdf"), "source asset").expect("source pdf");
        let service = BuildService::new(MergedConfig {
            target: Some("book".to_string()),
            ..Default::default()
        });

        service.clean(project.path(), true).expect("distclean");
        assert!(!project.path().join("auto").exists());
        assert!(!project.path().join("book.aux").exists());
        assert!(project.path().join("reference.pdf").is_file());
    }

    #[test]
    fn clean_rejects_output_directories_that_escape_the_project() {
        let project = tempfile::tempdir().expect("project");
        let service = BuildService::new(MergedConfig {
            outdir: Some("../shared".to_string()),
            ..Default::default()
        });

        let error = service
            .clean_with_options(project.path(), CleanOptions::default())
            .expect_err("unsafe outdir");
        assert!(error.to_string().contains("escapes the project root"));
    }
}
