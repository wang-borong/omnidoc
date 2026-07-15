use crate::build::executor::BuildExecutor;
use crate::build::pandoc::{PandocBuilder, PandocCommandProfile};
use crate::build::pandoc_policy::PandocOutputKind;
use crate::config::MergedConfig;
use crate::constants::{file_names, pandoc};
use crate::doc::templates::{generate_markdown_template, TemplateDocType};
use crate::error::{OmniDocError, Result};
use crate::utils::fs;
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
                let author = self.config.author.as_deref().unwrap_or("Unknown Author");

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
        let builder = PandocBuilder::new(self.config.clone())?;
        let options = builder.build_command_options(
            &effective_input,
            &output_path,
            PandocOutputKind::Pdf,
            &PandocCommandProfile::StandalonePdf { use_cn },
        )?;

        // 执行转换
        let args: Vec<&str> = options.iter().map(|s| s.as_str()).collect();
        self.executor
            .execute_in_dir(pandoc::CMD, &args[..], false, input.parent())?;

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
        let builder = PandocBuilder::new(self.config.clone())?;
        let options = builder.build_command_options(
            input,
            &output_path,
            PandocOutputKind::Html,
            &PandocCommandProfile::StandaloneHtml {
                css: css.map(Path::to_path_buf),
            },
        )?;

        // 执行转换
        let args: Vec<&str> = options.iter().map(|s| s.as_str()).collect();
        self.executor
            .execute_in_dir(pandoc::CMD, &args[..], false, input.parent())?;

        Ok(())
    }
}
