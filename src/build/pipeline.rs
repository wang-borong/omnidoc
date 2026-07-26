use crate::config::MergedConfig;
use crate::error::Result;
use std::path::Path;

/// 构建管道 trait
/// 定义构建流程的通用接口
pub trait BuildPipeline {
    /// 执行构建
    fn build(&self, project_path: &Path, verbose: bool) -> Result<()>;

    /// 检测项目类型
    fn detect_project_type(&self, project_path: &Path) -> Result<ProjectType>;
}

/// 项目类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectType {
    Markdown,
    Latex,
    Unknown,
}

impl ProjectType {
    /// 从文件扩展名判断项目类型
    pub fn from_entry_file(path: &Path) -> Self {
        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            match ext.to_lowercase().as_str() {
                "md" => ProjectType::Markdown,
                "tex" => ProjectType::Latex,
                _ => ProjectType::Unknown,
            }
        } else {
            ProjectType::Unknown
        }
    }
}

/// Resolve the project type using the same precedence as the build service:
/// an existing configured entry, then `project.from`, then conventional
/// Markdown/LaTeX entry files.
pub fn detect_project_type(config: &MergedConfig, project_path: &Path) -> ProjectType {
    if let Some(entry) = &config.entry {
        let entry_path = project_path.join(entry);
        if entry_path.exists() {
            return ProjectType::from_entry_file(&entry_path);
        }
    }

    if let Some(from) = &config.from {
        match from.trim().to_ascii_lowercase().as_str() {
            "markdown" | "md" => return ProjectType::Markdown,
            "latex" | "tex" => return ProjectType::Latex,
            _ => {}
        }
    }

    if project_path.join("main.md").exists() {
        return ProjectType::Markdown;
    }
    if project_path.join("main.tex").exists() {
        return ProjectType::Latex;
    }

    if let Ok(entries) = std::fs::read_dir(project_path) {
        if entries.flatten().any(|entry| {
            let path = entry.path();
            path.is_file()
                && path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("tex"))
        }) {
            return ProjectType::Latex;
        }
    }

    ProjectType::Unknown
}

#[cfg(test)]
mod tests {
    use super::{detect_project_type, ProjectType};
    use crate::config::MergedConfig;
    use std::fs;

    #[test]
    fn configured_entry_takes_precedence_over_conflicting_from_value() {
        let project = tempfile::tempdir().expect("project");
        fs::write(project.path().join("main.md"), "# Markdown\n").expect("entry");
        let config = MergedConfig {
            entry: Some("main.md".to_string()),
            from: Some("latex".to_string()),
            ..Default::default()
        };

        assert_eq!(
            detect_project_type(&config, project.path()),
            ProjectType::Markdown
        );
    }

    #[test]
    fn conventional_main_tex_is_detected_without_explicit_config() {
        let project = tempfile::tempdir().expect("project");
        fs::write(
            project.path().join("main.tex"),
            "\\documentclass{article}\n",
        )
        .expect("entry");

        assert_eq!(
            detect_project_type(&MergedConfig::default(), project.path()),
            ProjectType::Latex
        );
    }
}
