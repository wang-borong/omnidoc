use crate::build::executor::BuildExecutor;
use crate::build::pipeline::{BuildPipeline, ProjectType};
use crate::config::MergedConfig;
use crate::diagnostics::summarize_latex_log;
use crate::doc::services::FigureService;
use crate::error::{OmniDocError, Result};
use crate::latex_recorder;
use crate::project_tools::LATEX_INPUT_DEPFILE;
use crate::utils::directories::data_local_dir;
use crate::utils::fs;
use regex::Regex;
use std::collections::{BTreeMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

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
            if path_str == "drawio" || fs::exists(search_path.join("drawio")) {
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
            if path_str == "dac" || fs::exists(search_path.join("dac")) {
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

    /// 生成被 LaTeX 文档引用的派生图片。
    fn generate_figures(&self, project_path: &Path, verbose: bool) -> Result<()> {
        let (drawio_files, dot_mmd_files, json_files) = self.find_figure_sources(project_path);

        // 如果没有任何图片源文件，跳过
        if drawio_files.is_empty() && dot_mmd_files.is_empty() && json_files.is_empty() {
            return Ok(());
        }

        let figures_dir = self
            .config
            .figure_output
            .as_ref()
            .map(|s| project_path.join(s))
            .unwrap_or_else(|| project_path.join(&self.config.paths.figures_dir));

        if !fs::exists(&figures_dir) {
            fs::create_dir_all(&figures_dir)?;
        }

        let referenced_figures = self.referenced_figure_names(project_path);
        let mut sources = Vec::new();
        sources.extend(Self::filter_referenced_figure_sources(
            &json_files,
            &referenced_figures,
        ));
        sources.extend(Self::filter_referenced_figure_sources(
            &drawio_files,
            &referenced_figures,
        ));
        sources.extend(Self::filter_referenced_figure_sources(
            &dot_mmd_files
                .into_iter()
                .filter(|path| {
                    matches!(
                        path.extension()
                            .and_then(|extension| extension.to_str())
                            .map(|extension| extension.to_ascii_lowercase())
                            .as_deref(),
                        Some("dot" | "gv")
                    )
                })
                .collect::<Vec<_>>(),
            &referenced_figures,
        ));

        if sources.is_empty() {
            if verbose {
                println!("No referenced generated figures found.");
            }
            return Ok(());
        }

        if verbose {
            println!(
                "Generating {} referenced figure source(s) into {}",
                sources.len(),
                figures_dir.display()
            );
        }

        let figure_service = FigureService::new(self.config.clone())?;
        figure_service.generate_figures(
            project_path,
            &sources,
            Some(&figures_dir),
            Some("pdf"),
            false,
            None,
        )?;

        Ok(())
    }

    fn filter_referenced_figure_sources(
        sources: &[PathBuf],
        referenced_figures: &HashSet<String>,
    ) -> Vec<PathBuf> {
        sources
            .iter()
            .filter(|path| {
                if referenced_figures.is_empty() {
                    return true;
                }
                path.file_stem()
                    .and_then(|stem| stem.to_str())
                    .map(|stem| referenced_figures.contains(stem))
                    .unwrap_or(false)
            })
            .cloned()
            .collect()
    }

    fn referenced_figure_names(&self, project_path: &Path) -> HashSet<String> {
        let mut names = HashSet::new();
        let Ok(figure_re) =
            Regex::new(r#"\\(?:Figure|includegraphics)(?:\[[^\]]*\])?\{(?P<name>[^{}]+)\}"#)
        else {
            return names;
        };

        for entry in WalkDir::new(project_path)
            .into_iter()
            .filter_entry(|entry| Self::should_scan_tex_path(entry.path(), project_path))
            .flatten()
            .filter(|entry| {
                entry.file_type().is_file()
                    && entry.path().extension().and_then(|value| value.to_str()) == Some("tex")
            })
        {
            let Ok(content) = std::fs::read_to_string(entry.path()) else {
                continue;
            };
            for capture in figure_re.captures_iter(&content) {
                let Some(name) = capture.name("name").map(|value| value.as_str().trim()) else {
                    continue;
                };
                if name.is_empty() {
                    continue;
                }
                let stem = Path::new(name)
                    .file_stem()
                    .and_then(|value| value.to_str())
                    .unwrap_or(name);
                names.insert(stem.to_string());
            }
        }

        names
    }

    fn should_scan_tex_path(path: &Path, root: &Path) -> bool {
        if path == root {
            return true;
        }
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("");
        !matches!(
            name,
            ".git" | "build" | "target" | ".target" | ".cache" | ".omnidoc-cache" | "node_modules"
        )
    }

    fn ensure_bibliography_tools(&self, project_path: &Path) -> Result<()> {
        if !self.project_uses_biber(project_path) {
            return Ok(());
        }

        self.executor.check_tool("biber").map(|_| ()).map_err(|_| {
            OmniDocError::Project(
                "This LaTeX project uses biblatex/\\printbibliography, but the 'biber' command was not found. Install biber (for example texlive-biber or the equivalent TeX Live package) and rebuild.".to_string(),
            )
        })
    }

    fn project_uses_biber(&self, project_path: &Path) -> bool {
        for entry in WalkDir::new(project_path)
            .into_iter()
            .filter_entry(|entry| Self::should_scan_tex_path(entry.path(), project_path))
            .flatten()
            .filter(|entry| {
                entry.file_type().is_file()
                    && entry.path().extension().and_then(|value| value.to_str()) == Some("tex")
            })
        {
            let Ok(content) = std::fs::read_to_string(entry.path()) else {
                continue;
            };
            let lower = content.to_ascii_lowercase();
            if lower.contains("backend=bibtex") {
                continue;
            }
            if lower.contains("\\printbibliography")
                || lower.contains("\\addbibresource")
                || lower.contains("\\usepackage{biblatex}")
                || lower.contains("]{biblatex}")
            {
                return true;
            }
        }

        false
    }

    /// 构建 latexmk 选项
    fn build_latexmk_options(
        &self,
        entry_file: &Path,
        target_name: &str,
        verbose: bool,
    ) -> Vec<String> {
        let mut options = Vec::new();

        // OmniDoc already decides whether this pipeline needs to run from its
        // own dependency cache. Once it does, require latexmk to execute at
        // least one pass. Otherwise latexmk can keep a failed .fdb_latexmk
        // state and report "Nothing to do" even after the environment or a
        // missing dependency has been fixed.
        options.push("-g".to_string());

        // Quiet 模式（默认）
        if !verbose {
            options.push("-quiet".to_string());
        }

        // Job name (输出文件名)
        options.push(format!("-jobname={}", target_name));
        options.push("-recorder".to_string());

        // 输入文件
        options.push(entry_file.to_string_lossy().to_string());

        options
    }

    fn build_tectonic_options(&self, entry_file: &Path, outdir: &Path) -> Vec<String> {
        vec![
            "--outdir".to_string(),
            outdir.to_string_lossy().to_string(),
            "--keep-logs".to_string(),
            "--keep-intermediates".to_string(),
            entry_file.to_string_lossy().to_string(),
        ]
    }

    fn build_engine_options(
        &self,
        entry_file: &Path,
        outdir: &Path,
        target_name: &str,
    ) -> Vec<String> {
        vec![
            "-interaction=nonstopmode".to_string(),
            "-halt-on-error".to_string(),
            "-file-line-error".to_string(),
            "-shell-escape".to_string(),
            "-recorder".to_string(),
            format!("-output-directory={}", outdir.display()),
            format!("-jobname={}", target_name),
            entry_file.to_string_lossy().to_string(),
        ]
    }

    fn is_tectonic_engine(engine: &str) -> bool {
        Path::new(engine)
            .file_stem()
            .and_then(|name| name.to_str())
            .map(|name| name.eq_ignore_ascii_case("tectonic"))
            .unwrap_or_else(|| engine.eq_ignore_ascii_case("tectonic"))
    }

    fn find_latex_log_summary(
        &self,
        project_path: &Path,
        outdir: &Path,
        entry_file: &Path,
        target_name: &str,
    ) -> Option<String> {
        let entry_stem = entry_file
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or(target_name);
        let candidates = [
            outdir.join(format!("{}.log", target_name)),
            project_path.join(format!("{}.log", target_name)),
            outdir.join(format!("{}.log", entry_stem)),
            project_path.join(format!("{}.log", entry_stem)),
        ];

        for log_path in candidates {
            if let Some(summary) = summarize_latex_log(&log_path) {
                return Some(format!(
                    "LaTeX log summary ({}):\n{}",
                    log_path.display(),
                    summary
                ));
            }
        }

        None
    }

    fn copy_tectonic_output(
        &self,
        entry_file: &Path,
        outdir: &Path,
        target_name: &str,
    ) -> Result<()> {
        let entry_stem = entry_file
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or(target_name);
        let produced_pdf = outdir.join(format!("{}.pdf", entry_stem));
        let target_pdf = outdir.join(format!("{}.pdf", target_name));

        if produced_pdf != target_pdf && fs::exists(&produced_pdf) {
            fs::copy(&produced_pdf, &target_pdf)?;
        }

        Ok(())
    }

    fn run_engine_until_stable(
        &self,
        entry_file: &Path,
        outdir: &Path,
        target_name: &str,
        verbose: bool,
    ) -> Result<()> {
        let options = self.build_engine_options(entry_file, outdir, target_name);
        let args: Vec<&str> = options.iter().map(|s| s.as_str()).collect();
        let max_passes = self.config.max_latex_passes.max(1);
        let mut previous_hashes: Option<BTreeMap<String, u64>> = None;

        for pass in 1..=max_passes {
            if verbose {
                println!("LaTeX engine pass {}/{}", pass, max_passes);
            }

            self.executor.execute("latex_engine", &args[..], verbose)?;
            let current_hashes = self.collect_aux_hashes(outdir)?;

            if current_hashes.is_empty() {
                if verbose {
                    println!("No tracked LaTeX auxiliary files found; stopping after one pass.");
                }
                return Ok(());
            }

            if previous_hashes
                .as_ref()
                .map(|previous| previous == &current_hashes)
                .unwrap_or(false)
            {
                if verbose {
                    println!("LaTeX auxiliary files stabilized after {} passes.", pass);
                }
                return Ok(());
            }

            previous_hashes = Some(current_hashes);
        }

        if verbose {
            println!(
                "Reached max LaTeX passes ({}); continuing with latest output.",
                max_passes
            );
        }
        Ok(())
    }

    fn collect_aux_hashes(&self, outdir: &Path) -> Result<BTreeMap<String, u64>> {
        let mut hashes = BTreeMap::new();
        if !fs::exists(outdir) {
            return Ok(hashes);
        }

        for entry in fs::read_dir(outdir)? {
            let entry = entry?;
            let path = entry.path();
            if !fs::is_file(&path) || !Self::is_tracked_aux_file(&path) {
                continue;
            }

            let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            let bytes = std::fs::read(&path)?;
            let mut hasher = DefaultHasher::new();
            bytes.hash(&mut hasher);
            hashes.insert(name.to_string(), hasher.finish());
        }

        Ok(hashes)
    }

    fn is_tracked_aux_file(path: &Path) -> bool {
        let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
            return false;
        };

        matches!(
            extension.to_ascii_lowercase().as_str(),
            "aux" | "toc" | "lof" | "lot" | "out" | "bcf"
        ) || path
            .file_name()
            .and_then(|value| value.to_str())
            .map(|name| name.ends_with(".run.xml"))
            .unwrap_or(false)
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
        std::env::set_var("OUTDIR", &outdir);

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
        self.ensure_bibliography_tools(project_path)?;

        let latex_engine = self.executor.check_tool("latex_engine")?;
        let use_tectonic = Self::is_tectonic_engine(&latex_engine);
        let latex_backend = if self.config.latex_backend.trim().is_empty() {
            "latexmk".to_string()
        } else {
            self.config.latex_backend.to_ascii_lowercase()
        };
        if latex_backend != "latexmk" && latex_backend != "engine" {
            return Err(OmniDocError::Config(format!(
                "Unsupported LaTeX backend '{}'. Supported values: latexmk, engine",
                self.config.latex_backend
            )));
        }

        // 切换到项目目录执行（latexmk 需要相对路径）
        let original_dir = std::env::current_dir()?;
        std::env::set_current_dir(project_path)?;

        let result = if use_tectonic {
            let options = self.build_tectonic_options(&entry_file, &outdir);
            let args: Vec<&str> = options.iter().map(|s| s.as_str()).collect();
            self.executor.execute("latex_engine", &args[..], verbose)
        } else if latex_backend == "engine" {
            self.run_engine_until_stable(&entry_file, &outdir, &target_name, verbose)
        } else {
            let options = self.build_latexmk_options(&entry_file, &target_name, verbose);
            let args: Vec<&str> = options.iter().map(|s| s.as_str()).collect();
            self.executor.execute("latexmk", &args[..], verbose)
        };

        // 恢复原目录
        std::env::set_current_dir(original_dir)?;

        // 如果构建失败，尝试清理
        if let Err(err) = result {
            // latexmk cleanup may remove or truncate the log, so preserve its
            // useful diagnostics before cleaning generated files.
            let log_summary =
                self.find_latex_log_summary(project_path, &outdir, &entry_file, &target_name);

            if verbose && !use_tectonic && latex_backend == "latexmk" {
                println!("⚠ Build failed, attempting to clean...");
            }
            if !use_tectonic && latex_backend == "latexmk" {
                let jobname_arg = format!("-jobname={}", target_name);
                let clean_options = ["-c", &jobname_arg, entry_file.to_str().unwrap_or("")];
                let clean_args: Vec<&str> = clean_options.to_vec();
                let _ = self.executor.execute("latexmk", &clean_args[..], false);
            }

            let mut message = err.to_string();
            if let Some(log_summary) = log_summary {
                message.push_str("\n\n");
                message.push_str(&log_summary);
            }
            return Err(OmniDocError::Project(message));
        }

        if use_tectonic {
            self.copy_tectonic_output(&entry_file, &outdir, &target_name)?;
        }

        let cache_dir = project_path.join(".omnidoc-cache");
        let depfile = cache_dir.join(LATEX_INPUT_DEPFILE);
        let entry_stem = entry_file.file_stem().and_then(|name| name.to_str());
        let recorder = [
            outdir.join(format!("{target_name}.fls")),
            project_path.join(format!("{target_name}.fls")),
            entry_stem
                .map(|stem| outdir.join(format!("{stem}.fls")))
                .unwrap_or_default(),
        ]
        .into_iter()
        .find(|path| path.is_file());
        if let Some(recorder) = recorder {
            latex_recorder::write_depfile_from_fls(
                &recorder,
                &depfile,
                std::slice::from_ref(&outdir),
            )?;
        } else if depfile.exists() {
            fs::remove_file(&depfile)?;
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

#[cfg(test)]
mod tests {
    use super::LatexBuilder;
    use crate::config::MergedConfig;
    use std::path::Path;

    #[test]
    fn latexmk_rebuilds_after_a_cached_failed_invocation() {
        let builder = LatexBuilder::new(MergedConfig::default()).expect("latex builder");
        let options = builder.build_latexmk_options(Path::new("main.tex"), "book", false);

        assert!(options.iter().any(|option| option == "-g"));
        assert!(options.iter().any(|option| option == "-quiet"));
    }
}
