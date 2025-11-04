use crate::build::{BuildPipeline, LatexBuilder, PandocBuilder, ProjectType};
use crate::config::MergedConfig;
use crate::error::{OmniDocError, Result};
use std::path::Path;

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
        // 优先检查配置中的 entry 和 from 字段
        if let Some(entry) = &self.config.entry {
            let entry_path = project_path.join(entry);
            if entry_path.exists() {
                return Ok(ProjectType::from_entry_file(&entry_path));
            }
        }

        if let Some(from) = &self.config.from {
            match from.to_lowercase().as_str() {
                "markdown" | "md" => return Ok(ProjectType::Markdown),
                "latex" | "tex" => return Ok(ProjectType::Latex),
                _ => {}
            }
        }

        // 尝试使用 PandocBuilder 检测
        let pandoc_builder = PandocBuilder::new(self.config.clone())?;
        let pandoc_type = BuildPipeline::detect_project_type(&pandoc_builder, project_path)?;
        if pandoc_type != ProjectType::Unknown {
            return Ok(pandoc_type);
        }

        // 尝试使用 LatexBuilder 检测
        let latex_builder = LatexBuilder::new(self.config.clone())?;
        BuildPipeline::detect_project_type(&latex_builder, project_path)
    }

    /// 清理构建产物
    pub fn clean(&self, project_path: &Path, distclean: bool) -> Result<()> {
        use std::fs;
        use std::path::PathBuf;

        // 确定输出目录
        let outdir = self.config.outdir.as_ref()
            .map(|s| project_path.join(s))
            .unwrap_or_else(|| project_path.join("build"));

        // 清理输出目录
        if outdir.exists() {
            if distclean {
                fs::remove_dir_all(&outdir)?;
            } else {
                // 只清理 LaTeX 临时文件
                let patterns = ["*.aux", "*.log", "*.out", "*.synctex.gz", "*.toc", "*.fdb_latexmk", "*.fls"];
                for pattern in &patterns {
                    // 使用 glob 查找文件（简化实现）
                    if let Ok(entries) = fs::read_dir(&outdir) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                                if pattern.replace("*", "").is_empty() || name.ends_with(&pattern[2..]) {
                                    let _ = fs::remove_file(&path);
                                }
                            }
                        }
                    }
                }
            }
        }

        // distclean 时清理项目根目录的临时文件
        if distclean {
            let patterns = ["*.aux", "*.log", "*.out", "*.pdf", "*.synctex.gz", "*.toc"];
            for pattern in &patterns {
                if let Ok(entries) = fs::read_dir(project_path) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file() {
                            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                                if pattern.replace("*", "").is_empty() || name.ends_with(&pattern[2..]) {
                                    let _ = fs::remove_file(&path);
                                }
                            }
                        }
                    }
                }
            }

            // 清理 auto 目录
            let auto_dir = project_path.join("auto");
            if auto_dir.exists() {
                let _ = fs::remove_dir_all(&auto_dir);
            }
        }

        Ok(())
    }
}

