use crate::build::executor::BuildExecutor;
use crate::build::pipeline::{BuildPipeline, ProjectType};
use crate::config::MergedConfig;
use crate::error::{OmniDocError, Result};
use crate::utils::fs;
use dirs::data_local_dir;
use std::path::{Path, PathBuf};

/// LaTeX 构建器
/// 实现 top.mk 的 latex 构建功能
pub struct LatexBuilder {
    executor: BuildExecutor,
    config: MergedConfig,
}

impl LatexBuilder {
    pub fn new(config: MergedConfig) -> Result<Self> {
        let executor = BuildExecutor::new(config.tool_paths.clone());
        Ok(Self { executor, config })
    }

    /// 查找图片源文件
    fn find_figure_sources(
        &self,
        project_path: &Path,
    ) -> (Vec<PathBuf>, Vec<PathBuf>, Vec<PathBuf>) {
        let mut drawio_files = Vec::new();
        let mut dot_mmd_files = Vec::new();
        let mut json_files = Vec::new();

        // 从配置的路径或默认路径查找
        let search_paths = if !self.config.figure_paths.is_empty() {
            self.config.figure_paths.clone()
        } else {
            vec![
                "drawio".to_string(),
                "dac".to_string(),
                "figure".to_string(),
                "figures".to_string(),
            ]
        };

        for path_str in search_paths {
            let search_path = project_path.join(&path_str);
            if !fs::exists(&search_path) {
                continue;
            }

            // 查找 drawio 文件
            if path_str == "drawio" || fs::exists(&search_path.join("drawio")) {
                let drawio_dir = if path_str == "drawio" {
                    search_path.clone()
                } else {
                    search_path.join("drawio")
                };
                if let Ok(entries) = fs::read_dir(&drawio_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().and_then(|s| s.to_str()) == Some("drawio") {
                            drawio_files.push(path);
                        }
                    }
                }
            }

            // 查找 dot 和 mmd 文件
            if path_str == "dac" || fs::exists(&search_path.join("dac")) {
                let dac_dir = if path_str == "dac" {
                    search_path.clone()
                } else {
                    search_path.join("dac")
                };
                if let Ok(entries) = fs::read_dir(&dac_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                            match ext {
                                "dot" | "mmd" => dot_mmd_files.push(path),
                                "json" => json_files.push(path),
                                _ => {}
                            }
                        }
                    }
                }
            }
        }

        (drawio_files, dot_mmd_files, json_files)
    }

    /// 生成图片
    /// 这部分功能将在后续的 FigureService 中实现
    /// 这里先提供一个占位实现
    fn generate_figures(&self, project_path: &Path, verbose: bool) -> Result<()> {
        let (drawio_files, dot_mmd_files, json_files) = self.find_figure_sources(project_path);

        // 如果没有任何图片源文件，跳过
        if drawio_files.is_empty() && dot_mmd_files.is_empty() && json_files.is_empty() {
            return Ok(());
        }

        // 创建输出目录
        let figures_dir = self
            .config
            .figure_output
            .as_ref()
            .map(|s| project_path.join(s))
            .unwrap_or_else(|| project_path.join("figures"));

        if !fs::exists(&figures_dir) {
            fs::create_dir_all(&figures_dir)?;
        }

        // TODO: 实现图片生成逻辑
        // 这里需要调用 figure-generator.py 和 bit-field.py
        // 暂时先跳过，将在 FigureService 中实现

        if verbose
            && (!drawio_files.is_empty() || !dot_mmd_files.is_empty() || !json_files.is_empty())
        {
            println!("ℹ Figure generation will be handled by FigureService");
        }

        Ok(())
    }

    /// 构建 latexmk 选项
    fn build_latexmk_options(
        &self,
        entry_file: &Path,
        target_name: &str,
        verbose: bool,
    ) -> Vec<String> {
        let mut options = Vec::new();

        // Quiet 模式（默认）
        if !verbose {
            options.push("-quiet".to_string());
        }

        // Job name (输出文件名)
        options.push("-jobname".to_string());
        options.push(target_name.to_string());

        // 输入文件
        options.push(entry_file.to_string_lossy().to_string());

        options
    }

    /// 设置 LaTeX 环境变量
    fn setup_latex_env(&self) -> Result<()> {
        use std::env;

        // 设置 TEXMFHOME
        if let Some(texmfhome) = &self.config.texmfhome {
            env::set_var("TEXMFHOME", texmfhome);
        } else {
            // 默认值
            let omnidoc_lib = self.config.lib_path.clone().unwrap_or_else(|| {
                data_local_dir()
                    .map(|d| {
                        d.join("omnidoc")
                            .join("texmf")
                            .to_string_lossy()
                            .to_string()
                    })
                    .unwrap_or_else(|| {
                        if let Some(h) = dirs::home_dir() {
                            h.join(".local")
                                .join("share")
                                .join("omnidoc")
                                .join("texmf")
                                .to_string_lossy()
                                .to_string()
                        } else {
                            ".local/share/omnidoc/texmf".to_string()
                        }
                    })
            });
            env::set_var("TEXMFHOME", &omnidoc_lib);
        }

        // 设置 TEXINPUTS
        if let Some(texinputs) = &self.config.texinputs {
            env::set_var("TEXINPUTS", texinputs);
        }

        // 设置 BIBINPUTS
        if let Some(bibinputs) = &self.config.bibinputs {
            env::set_var("BIBINPUTS", bibinputs);
        }

        Ok(())
    }
}

impl BuildPipeline for LatexBuilder {
    fn build(&self, project_path: &Path, verbose: bool) -> Result<()> {
        // 设置环境变量
        self.setup_latex_env()?;

        // 确定入口文件
        let entry_file = if let Some(entry) = &self.config.entry {
            project_path.join(entry)
        } else {
            // 尝试查找 main.tex
            let main_tex = project_path.join("main.tex");
            if fs::exists(&main_tex) {
                main_tex
            } else {
                // 尝试查找任何 .tex 文件
                let mut found = None;
                if let Ok(entries) = fs::read_dir(project_path) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if fs::is_file(&path)
                            && path.extension().and_then(|s| s.to_str()) == Some("tex")
                        {
                            if path.file_stem().and_then(|s| s.to_str()) == Some("main") {
                                found = Some(path);
                                break;
                            } else if found.is_none() {
                                found = Some(path);
                            }
                        }
                    }
                }
                found.ok_or_else(|| OmniDocError::Project(
                    "No LaTeX entry file found. Please specify 'entry' in .omnidoc.toml or create main.tex".to_string()
                ))?
            }
        };

        if !fs::exists(&entry_file) {
            return Err(OmniDocError::Project(format!(
                "Entry file not found: {}",
                entry_file.display()
            )));
        }

        // 确定输出目录
        let outdir = self
            .config
            .outdir
            .as_ref()
            .map(|s| project_path.join(s))
            .unwrap_or_else(|| project_path.join("build"));

        // 创建输出目录
        if !fs::exists(&outdir) {
            fs::create_dir_all(&outdir)?;
        }

        // 确定目标名称
        let target_name = self.config.target.as_ref().cloned().unwrap_or_else(|| {
            project_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("document")
                .to_string()
        });

        // 生成图片（如果有源文件）
        // 注意：这里只是检查，实际生成将在 FigureService 中实现
        let (drawio, dot_mmd, json) = self.find_figure_sources(project_path);
        if !drawio.is_empty() || !dot_mmd.is_empty() || !json.is_empty() {
            self.generate_figures(project_path, verbose)?;
        }

        // 构建 latexmk 命令
        let options = self.build_latexmk_options(&entry_file, &target_name, verbose);

        // 切换到项目目录执行（latexmk 需要相对路径）
        let original_dir = std::env::current_dir()?;
        std::env::set_current_dir(project_path)?;

        // 执行 latexmk
        let args: Vec<&str> = options.iter().map(|s| s.as_str()).collect();
        let result = self.executor.execute("latexmk", &args[..], verbose);

        // 恢复原目录
        std::env::set_current_dir(original_dir)?;

        // 如果构建失败，尝试清理
        if result.is_err() {
            if verbose {
                println!("⚠ Build failed, attempting to clean...");
            }
            let clean_options = vec![
                "-c",
                "-jobname",
                &target_name,
                entry_file.to_str().unwrap_or(""),
            ];
            let clean_args: Vec<&str> = clean_options.iter().map(|s| *s).collect();
            let _ = self.executor.execute("latexmk", &clean_args[..], false);
            return result;
        }

        // 检查输出文件
        let output_file = outdir.join(format!("{}.pdf", target_name));
        if !fs::exists(&output_file) {
            // 可能输出在项目根目录
            let alt_output = project_path.join(format!("{}.pdf", target_name));
            if fs::exists(&alt_output) {
                if verbose {
                    println!("✓ Built PDF: {}", alt_output.display());
                }
            } else {
                return Err(OmniDocError::Project(format!(
                    "PDF output not found. Expected at {} or {}",
                    output_file.display(),
                    alt_output.display()
                )));
            }
        } else {
            if verbose {
                println!("✓ Built PDF: {}", output_file.display());
            }
        }

        Ok(())
    }

    fn detect_project_type(&self, project_path: &Path) -> Result<ProjectType> {
        // 检查入口文件
        if let Some(entry) = &self.config.entry {
            let entry_path = project_path.join(entry);
            if fs::exists(&entry_path) {
                return Ok(ProjectType::from_entry_file(&entry_path));
            }
        }

        // 尝试查找 main.tex
        let main_tex = project_path.join("main.tex");
        if fs::exists(&main_tex) {
            return Ok(ProjectType::Latex);
        }

        // 尝试查找任何 .tex 文件
        if let Ok(entries) = fs::read_dir(project_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if fs::is_file(&path) && path.extension().and_then(|s| s.to_str()) == Some("tex") {
                    return Ok(ProjectType::Latex);
                }
            }
        }

        Ok(ProjectType::Unknown)
    }
}
