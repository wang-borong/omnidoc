use crate::build::executor::BuildExecutor;
use crate::build::pipeline::{BuildPipeline, ProjectType};
use crate::config::MergedConfig;
use crate::error::{OmniDocError, Result};
use dirs::data_local_dir;
use std::path::{Path, PathBuf};
use std::fs;

/// Pandoc 构建器
/// 实现 pandoc.mk 的功能
pub struct PandocBuilder {
    executor: BuildExecutor,
    config: MergedConfig,
}

impl PandocBuilder {
    pub fn new(config: MergedConfig) -> Result<Self> {
        let executor = BuildExecutor::new(config.tool_paths.clone());
        Ok(Self { executor, config })
    }

    /// 构建 Pandoc 选项
    fn build_pandoc_options(&self, project_path: &Path, entry_file: &Path, output_file: &Path) -> Vec<String> {
        let mut options = Vec::new();

        // 基础格式选项（从配置获取，默认 markdown+east_asian_line_breaks+footnotes）
        options.push("-f".to_string());
        let from_format = self.config.pandoc_from_format.clone()
            .unwrap_or_else(|| "markdown+east_asian_line_breaks+footnotes".to_string());
        options.push(from_format);

        // 获取 omnidoc-libs 路径
        let omnidoc_lib = self.config.lib_path.clone()
            .unwrap_or_else(|| {
                data_local_dir()
                    .map(|d| d.join("omnidoc").to_string_lossy().to_string())
                    .unwrap_or_else(|| "$HOME/.local/share/omnidoc".to_string())
            });

        // Lua filters（从配置获取或使用默认值）
        let default_filters = vec![
            "include-files.lua".to_string(),
            "include-code-files.lua".to_string(),
            "diagram-generator.lua".to_string(),
            "ltblr.lua".to_string(),
            "latex-patch.lua".to_string(),
            "fonts-and-alignment.lua".to_string(),
        ];
        let filters = if !self.config.pandoc_lua_filters.is_empty() {
            &self.config.pandoc_lua_filters
        } else {
            &default_filters
        };

        for filter in filters {
            options.push("--lua-filter".to_string());
            let filter_path = format!("{}/pandoc/data/filters/{}", omnidoc_lib, filter);
            options.push(filter_path);
        }

        // Python path（从配置获取，默认 python3）
        options.push("--metadata".to_string());
        let python_path = self.config.pandoc_python_path.clone()
            .unwrap_or_else(|| "python3".to_string());
        options.push(format!("pythonPath:{}", python_path));

        // Pandoc plugins
        options.push("-F".to_string());
        options.push("pandoc-crossref".to_string());
        options.push("--citeproc".to_string());

        // PDF engine（从配置获取，默认 xelatex）
        options.push("--pdf-engine".to_string());
        let latex_engine = self.executor.check_tool("latex_engine")
            .unwrap_or_else(|_| "xelatex".to_string());
        options.push(latex_engine);

        // Syntax highlighting（从配置获取，默认 idiomatic）
        options.push("--syntax-highlighting".to_string());
        let syntax_highlighting = self.config.pandoc_syntax_highlighting.clone()
            .unwrap_or_else(|| "idiomatic".to_string());
        options.push(syntax_highlighting);

        // Data directory（从配置获取或使用默认值）
        options.push("--data-dir".to_string());
        let data_dir = self.config.pandoc_data_dir.clone()
            .unwrap_or_else(|| format!("{}/pandoc/data", omnidoc_lib));
        options.push(data_dir);

        // Standalone（从配置获取，默认 true）
        if self.config.pandoc_standalone {
            options.push("--standalone".to_string());
        }

        // Embed resources（从配置获取，默认 true）
        if self.config.pandoc_embed_resources {
            options.push("--embed-resources".to_string());
        }

        // Resource path（从配置获取或使用默认值）
        options.push("--resource-path".to_string());
        let resource_path = if !self.config.pandoc_resource_path.is_empty() {
            self.config.pandoc_resource_path.join(":")
        } else {
            format!(".:{}:image:images:figure:figures:biblio", format!("{}/pandoc/csl", omnidoc_lib))
        };
        options.push(resource_path);

        // Template（从配置获取，默认 pantext.latex）
        options.push("--template".to_string());
        let template = self.config.pandoc_template.clone()
            .unwrap_or_else(|| "pantext.latex".to_string());
        options.push(template);

        // Metadata file (如果配置了)
        if let Some(metadata_file) = &self.config.metadata_file {
            options.push("--metadata-file".to_string());
            options.push(metadata_file.clone());
        }

        // Crossref YAML（如果配置了语言且不是英文）
        if let Some(lang) = &self.config.pandoc_lang {
            if lang != "en" {
                let crossref_yaml = self.config.pandoc_crossref_yaml.clone()
                    .unwrap_or_else(|| format!("{}/pandoc/crossref.yaml", omnidoc_lib));
                options.push("-M".to_string());
                options.push(format!("crossrefYaml={}", crossref_yaml));
            }
        }

        // Verbose 模式
        if self.config.verbose {
            options.push("--verbose".to_string());
        }

        // 添加项目配置的额外选项
        options.extend(self.config.pandoc_options.clone());

        // 输入文件
        options.push(entry_file.to_string_lossy().to_string());

        // 输出文件
        options.push("-o".to_string());
        options.push(output_file.to_string_lossy().to_string());

        options
    }
}

impl BuildPipeline for PandocBuilder {
    fn build(&self, project_path: &Path, verbose: bool) -> Result<()> {
        // 确定入口文件
        let entry_file = if let Some(entry) = &self.config.entry {
            project_path.join(entry)
        } else {
            // 尝试查找 main.md
            let main_md = project_path.join("main.md");
            if main_md.exists() {
                main_md
            } else {
                return Err(OmniDocError::Project(
                    "No entry file found. Please specify 'entry' in .omnidoc.toml or create main.md".to_string()
                ));
            }
        };

        if !entry_file.exists() {
            return Err(OmniDocError::Project(format!(
                "Entry file not found: {}",
                entry_file.display()
            )));
        }

        // 确定输出目录和文件名
        let outdir = self.config.outdir.as_ref()
            .map(|s| project_path.join(s))
            .unwrap_or_else(|| project_path.join("build"));

        // 创建输出目录
        if !outdir.exists() {
            fs::create_dir_all(&outdir)?;
        }

        // 确定输出文件名
        let target_name = self.config.target.as_ref()
            .cloned()
            .unwrap_or_else(|| {
                project_path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("document")
                    .to_string()
            });

        let output_file = outdir.join(format!("{}.pdf", target_name));

        // 构建 Pandoc 命令
        let options = self.build_pandoc_options(project_path, &entry_file, &output_file);

        // 执行构建
        let args: Vec<&str> = options.iter().map(|s| s.as_str()).collect();
        self.executor.execute("pandoc", &args[..], verbose)?;

        if verbose {
            println!("✓ Built PDF: {}", output_file.display());
        }

        Ok(())
    }

    fn detect_project_type(&self, project_path: &Path) -> Result<ProjectType> {
        // 检查入口文件
        if let Some(entry) = &self.config.entry {
            let entry_path = project_path.join(entry);
            if entry_path.exists() {
                return Ok(ProjectType::from_entry_file(&entry_path));
            }
        }

        // 尝试查找 main.md 或 main.tex
        let main_md = project_path.join("main.md");
        if main_md.exists() {
            return Ok(ProjectType::Markdown);
        }

        let main_tex = project_path.join("main.tex");
        if main_tex.exists() {
            return Ok(ProjectType::Latex);
        }

        Ok(ProjectType::Unknown)
    }
}

