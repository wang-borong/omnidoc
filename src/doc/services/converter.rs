use crate::build::executor::BuildExecutor;
use crate::config::MergedConfig;
use crate::error::{OmniDocError, Result};
use dirs::data_local_dir;
use std::path::{Path, PathBuf};
use std::fs;

/// 格式转换服务
/// 提供 md2pdf 和 md2html 功能
pub struct ConverterService {
    executor: BuildExecutor,
    config: MergedConfig,
}

impl ConverterService {
    pub fn new(config: MergedConfig) -> Result<Self> {
        let executor = BuildExecutor::new(config.tool_paths.clone());
        Ok(Self { executor, config })
    }

    /// 将 Markdown 转换为 PDF
    pub fn md_to_pdf(&self, input: &Path, output: Option<&Path>) -> Result<()> {
        if !input.exists() {
            return Err(OmniDocError::Project(format!(
                "Input file not found: {}",
                input.display()
            )));
        }

        // 确定输出文件路径
        let output_path = if let Some(out) = output {
            out.to_path_buf()
        } else {
            // 与输入文件同目录，后缀改为 .pdf
            let mut out = input.to_path_buf();
            out.set_extension("pdf");
            out
        };

        // 构建 Pandoc 选项
        let options = self.build_pandoc_pdf_options(input, &output_path);

        // 执行转换
        let args: Vec<&str> = options.iter().map(|s| s.as_str()).collect();
        self.executor.execute("pandoc", &args[..], false)?;

        Ok(())
    }

    /// 将 Markdown 转换为 HTML
    pub fn md_to_html(&self, input: &Path, output: Option<&Path>, css: Option<&Path>) -> Result<()> {
        if !input.exists() {
            return Err(OmniDocError::Project(format!(
                "Input file not found: {}",
                input.display()
            )));
        }

        // 确定输出文件路径
        let output_path = if let Some(out) = output {
            out.to_path_buf()
        } else {
            // 与输入文件同目录，后缀改为 .html
            let mut out = input.to_path_buf();
            out.set_extension("html");
            out
        };

        // 构建 Pandoc 选项
        let options = self.build_pandoc_html_options(input, &output_path, css);

        // 执行转换
        let args: Vec<&str> = options.iter().map(|s| s.as_str()).collect();
        self.executor.execute("pandoc", &args[..], false)?;

        Ok(())
    }

    /// 构建 Pandoc PDF 选项
    fn build_pandoc_pdf_options(&self, input: &Path, output: &Path) -> Vec<String> {
        let mut options = Vec::new();

        // 基础格式（从配置获取，默认 markdown+east_asian_line_breaks+footnotes）
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

        // Crossref YAML（如果配置了语言且不是英文）
        if let Some(lang) = &self.config.pandoc_lang {
            if lang != "en" {
                let crossref_yaml = self.config.pandoc_crossref_yaml.clone()
                    .unwrap_or_else(|| format!("{}/pandoc/crossref.yaml", omnidoc_lib));
                options.push("-M".to_string());
                options.push(format!("crossrefYaml={}", crossref_yaml));
            }
        }

        // 添加配置的额外选项
        options.extend(self.config.pandoc_options.clone());

        // 输入和输出
        options.push(input.to_string_lossy().to_string());
        options.push("-o".to_string());
        options.push(output.to_string_lossy().to_string());

        options
    }

    /// 构建 Pandoc HTML 选项
    fn build_pandoc_html_options(&self, input: &Path, output: &Path, css: Option<&Path>) -> Vec<String> {
        let mut options = Vec::new();

        // 基础格式（从配置获取，默认 markdown）
        options.push("-f".to_string());
        let from_format = self.config.pandoc_from_format.clone()
            .unwrap_or_else(|| "markdown".to_string());
        options.push(from_format);
        options.push("-t".to_string());
        let to_format = self.config.pandoc_to_format.clone()
            .unwrap_or_else(|| "html".to_string());
        options.push(to_format);

        // 获取 omnidoc-libs 路径
        let omnidoc_lib = self.config.lib_path.clone()
            .unwrap_or_else(|| {
                data_local_dir()
                    .map(|d| d.join("omnidoc").to_string_lossy().to_string())
                    .unwrap_or_else(|| "$HOME/.local/share/omnidoc".to_string())
            });

        // Lua filters（从配置获取或使用默认值，HTML 使用较少的 filters）
        let default_filters = vec![
            "include-code-files.lua".to_string(),
            "include-files.lua".to_string(),
            "diagram-generator.lua".to_string(),
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

        // Crossref YAML（从配置获取或使用默认值）
        let crossref_yaml = self.config.pandoc_crossref_yaml.clone()
            .unwrap_or_else(|| format!("{}/pandoc/data/crossref.yaml", omnidoc_lib));
        options.push("-M".to_string());
        options.push(format!("crossrefYaml={}", crossref_yaml));

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

        // Resource path（从配置获取或使用默认值，HTML 使用不同的路径）
        options.push("--resource-path".to_string());
        let resource_path = if !self.config.pandoc_resource_path.is_empty() {
            self.config.pandoc_resource_path.join(":")
        } else {
            format!(".:{}:image:images:figure:figures:biblio", format!("{}/pandoc/headers", omnidoc_lib))
        };
        options.push(resource_path);

        // CSS（从参数、配置或默认值获取）
        let css_path = if let Some(css_file) = css {
            css_file.to_path_buf()
        } else if let Some(css_str) = &self.config.pandoc_css {
            PathBuf::from(css_str)
        } else {
            PathBuf::from(format!("{}/pandoc/css/advance-editor.css", omnidoc_lib))
        };

        if css_path.exists() {
            options.push("--css".to_string());
            options.push(css_path.to_string_lossy().to_string());
        }

        // 添加配置的额外选项
        options.extend(self.config.pandoc_options.clone());

        // 输入和输出
        options.push(input.to_string_lossy().to_string());
        options.push("-o".to_string());
        options.push(output.to_string_lossy().to_string());

        options
    }
}

