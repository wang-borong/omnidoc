use crate::build::executor::BuildExecutor;
use crate::config::MergedConfig;
use crate::constants::{file_names, pandoc};
use crate::doc::templates::{generate_markdown_template, TemplateDocType};
use crate::error::{OmniDocError, Result};
use crate::utils::fs;
use dirs::data_local_dir;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

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
    pub fn md_to_pdf(&self, input: &Path, output: Option<&Path>, lang: Option<&str>) -> Result<()> {
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
            out.set_extension(file_names::PDF_EXTENSION);
            out
        };

        // 如果输入 Markdown 没有 YAML 前言（--- 开头），则基于内置模板生成元数据头，
        // 写入临时文件：元数据头 + 原始内容，然后以该临时文件作为 Pandoc 输入
        let mut effective_input: PathBuf = input.to_path_buf();
        let mut temp_to_cleanup: Option<PathBuf> = None;
        let mut use_cn = false;
        if let Ok(content) = fs::read_to_string(input) {
            let trimmed = content.trim_start();
            let has_frontmatter = trimmed.starts_with("---\n") || trimmed.starts_with("---\r\n");
            if !has_frontmatter {
                let title = crate::utils::path::file_stem_str(input).unwrap_or("document");
                let author = self
                    .config
                    .author
                    .as_ref()
                    .map(|s| s.as_str())
                    .unwrap_or("Unknown Author");

                // 语言：默认中文（保持与 Python 默认一致）；英文时使用更简洁的头部
                use_cn = match lang {
                    Some(l) => l.eq_ignore_ascii_case("cn") || l.eq_ignore_ascii_case("zh"),
                    None => true,
                };

                let header = if use_cn {
                    // 使用 CTEXMD 模板生成与 Python 版本相近的元数据头
                    generate_markdown_template(title, author, TemplateDocType::CTEXMD)
                } else {
                    // 英文：复用内置 Markdown 模板（选择一个空 header/doctype 的类型）
                    // 这里选择 CTART 以触发空 header/doctype 分支
                    generate_markdown_template(title, author, TemplateDocType::CTART)
                };

                let merged = format!("{}\n{}", header, content);

                // 生成唯一的临时文件路径
                let ts = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis();
                let fname = format!("omnidoc_md2pdf_{}_{}.md", title, ts);
                let mut tmp_path = std::env::temp_dir();
                tmp_path.push(fname);

                fs::write(&tmp_path, merged.as_bytes())?;
                effective_input = tmp_path.clone();
                temp_to_cleanup = Some(tmp_path);
            }
        }

        // 构建 Pandoc 选项（可能使用临时合成的输入文件）
        let options = self.build_pandoc_pdf_options(&effective_input, &output_path, use_cn);

        // 执行转换
        let args: Vec<&str> = options.iter().map(|s| s.as_str()).collect();
        self.executor.execute(pandoc::CMD, &args[..], false)?;

        // 清理临时文件（如有）
        if let Some(tmp) = temp_to_cleanup {
            let _ = fs::remove_file(tmp);
        }

        Ok(())
    }

    /// 将 Markdown 转换为 HTML
    pub fn md_to_html(
        &self,
        input: &Path,
        output: Option<&Path>,
        css: Option<&Path>,
    ) -> Result<()> {
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
            out.set_extension(file_names::HTML_EXTENSION);
            out
        };

        // 构建 Pandoc 选项
        let options = self.build_pandoc_html_options(input, &output_path, css);

        // 执行转换
        let args: Vec<&str> = options.iter().map(|s| s.as_str()).collect();
        self.executor.execute(pandoc::CMD, &args[..], false)?;

        Ok(())
    }

    /// 构建 Pandoc PDF 选项
    fn build_pandoc_pdf_options(&self, input: &Path, output: &Path, use_cn: bool) -> Vec<String> {
        let mut options =
            self.build_pandoc_common_options(input, output, pandoc::DEFAULT_FROM_PDF, None);

        // PDF 专用选项
        // PDF engine（从配置获取，默认 xelatex）
        options.push(pandoc::FLAG_PDF_ENGINE.to_string());
        let latex_engine = self
            .executor
            .check_tool("latex_engine")
            .unwrap_or_else(|_| pandoc::DEFAULT_ENGINE_LATEX.to_string());
        options.push(latex_engine);

        // Syntax highlighting（从配置获取，默认 idiomatic）
        options.push(pandoc::FLAG_SYNTAX_HIGHLIGHTING.to_string());
        let syntax_highlighting = self
            .config
            .pandoc_syntax_highlighting
            .clone()
            .unwrap_or_else(|| pandoc::DEFAULT_SYNTAX.to_string());
        options.push(syntax_highlighting);

        // Template（从配置获取，默认 pantext.latex）
        options.push(pandoc::FLAG_TEMPLATE.to_string());
        let template = self
            .config
            .pandoc_template
            .clone()
            .unwrap_or_else(|| pandoc::DEFAULT_TEMPLATE_LATEX.to_string());
        options.push(template);

        // Citeproc（PDF 专用）
        options.push("--citeproc".to_string());

        if use_cn {
            let omnidoc_lib = self.get_omnidoc_lib_path();
            let crossref_yaml =
                self.config.pandoc_crossref_yaml.clone().unwrap_or_else(|| {
                    format!("{}/{}", omnidoc_lib, pandoc::LIB_PANDOC_CROSSREF_YAML)
                });
            options.push(pandoc::FLAG_META_SHORT.to_string());
            options.push(format!("crossrefYaml={}", crossref_yaml));
        }

        options
    }

    /// 构建 Pandoc HTML 选项
    fn build_pandoc_html_options(
        &self,
        input: &Path,
        output: &Path,
        css: Option<&Path>,
    ) -> Vec<String> {
        let mut options = self.build_pandoc_common_options(
            input,
            output,
            pandoc::DEFAULT_FROM_HTML,
            Some(pandoc::DEFAULT_TO_HTML),
        );

        // HTML 专用选项
        // Crossref YAML（从配置获取或使用默认值）
        let omnidoc_lib = self.get_omnidoc_lib_path();
        let crossref_yaml = self.config.pandoc_crossref_yaml.clone().unwrap_or_else(|| {
            format!("{}/{}", omnidoc_lib, pandoc::LIB_PANDOC_CROSSREF_YAML_HTML)
        });
        options.push(pandoc::FLAG_META_SHORT.to_string());
        options.push(format!("crossrefYaml={}", crossref_yaml));

        // Resource path（HTML 使用不同的路径）
        options.push(pandoc::FLAG_RESOURCE_PATH.to_string());
        let resource_path = if !self.config.pandoc_resource_path.is_empty() {
            self.config.pandoc_resource_path.join(":")
        } else {
            format!(
                ".:{}{}",
                format!("{}/{}", omnidoc_lib, pandoc::LIB_PANDOC_HEADERS),
                pandoc::RESOURCE_PATH_COMMON_SUFFIX
            )
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
            options.push(pandoc::FLAG_CSS.to_string());
            options.push(css_path.to_string_lossy().to_string());
        }

        options
    }

    /// 构建 Pandoc 公共选项
    fn build_pandoc_common_options(
        &self,
        input: &Path,
        output: &Path,
        default_from_format: &str,
        to_format: Option<&str>,
    ) -> Vec<String> {
        let mut options = Vec::new();

        // 基础格式（从配置获取或使用默认值）
        options.push(pandoc::FLAG_FROM.to_string());
        let from_format = self
            .config
            .pandoc_from_format
            .clone()
            .unwrap_or_else(|| default_from_format.to_string());
        options.push(from_format);

        // 输出格式（如果指定）
        if let Some(to) = to_format {
            options.push(pandoc::FLAG_TO.to_string());
            let format = self
                .config
                .pandoc_to_format
                .clone()
                .unwrap_or_else(|| to.to_string());
            options.push(format);
        }

        // 获取 omnidoc-libs 路径
        let omnidoc_lib = self.get_omnidoc_lib_path();

        // Lua filters（从配置获取或使用默认值）
        let default_filters = if to_format.is_some() {
            // HTML 使用较少的 filters
            vec![
                "include-code-files.lua".to_string(),
                "include-files.lua".to_string(),
                "diagram-generator.lua".to_string(),
                "fonts-and-alignment.lua".to_string(),
            ]
        } else {
            // PDF 使用所有 filters
            vec![
                "include-files.lua".to_string(),
                "include-code-files.lua".to_string(),
                "diagram-generator.lua".to_string(),
                "ltblr.lua".to_string(),
                "latex-patch.lua".to_string(),
                "fonts-and-alignment.lua".to_string(),
            ]
        };

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
        options.push(pandoc::FLAG_METADATA.to_string());
        let python_path = self
            .config
            .pandoc_python_path
            .clone()
            .unwrap_or_else(|| pandoc::DEFAULT_PYTHON.to_string());
        options.push(format!("pythonPath:{}", python_path));

        // Pandoc plugins
        options.push(pandoc::FLAG_FILTER.to_string());
        options.push(pandoc::DEFAULT_PLUGIN_CROSSREF.to_string());

        // Data directory（从配置获取或使用默认值）
        options.push(pandoc::FLAG_DATA_DIR.to_string());
        let data_dir = self
            .config
            .pandoc_data_dir
            .clone()
            .unwrap_or_else(|| format!("{}/{}", omnidoc_lib, pandoc::LIB_PANDOC_DATA));
        options.push(data_dir);

        // Standalone（从配置获取，默认 true）
        if self.config.pandoc_standalone {
            options.push(pandoc::FLAG_STANDALONE.to_string());
        }

        // Embed resources（从配置获取，默认 true）
        if self.config.pandoc_embed_resources {
            options.push(pandoc::FLAG_EMBED_RESOURCES.to_string());
        }

        // Resource path（PDF 使用，HTML 会覆盖）
        if to_format.is_none() {
            options.push(pandoc::FLAG_RESOURCE_PATH.to_string());
            let resource_path = if !self.config.pandoc_resource_path.is_empty() {
                self.config.pandoc_resource_path.join(":")
            } else {
                format!(
                    ".:{}{}",
                    format!("{}/{}", omnidoc_lib, pandoc::LIB_PANDOC_CSL),
                    pandoc::RESOURCE_PATH_COMMON_SUFFIX
                )
            };
            options.push(resource_path);
        }

        // 添加配置的额外选项
        options.extend(self.config.pandoc_options.clone());

        // 输入和输出
        options.push(input.to_string_lossy().to_string());
        options.push(pandoc::FLAG_OUTPUT.to_string());
        options.push(output.to_string_lossy().to_string());

        options
    }

    /// 获取 omnidoc-libs 路径
    fn get_omnidoc_lib_path(&self) -> String {
        self.config.lib_path.clone().unwrap_or_else(|| {
            if let Some(d) = data_local_dir() {
                d.join("omnidoc").to_string_lossy().to_string()
            } else if let Some(h) = dirs::home_dir() {
                h.join(".local")
                    .join("share")
                    .join("omnidoc")
                    .to_string_lossy()
                    .to_string()
            } else {
                ".local/share/omnidoc".to_string()
            }
        })
    }
}
