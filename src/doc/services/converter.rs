use crate::build::executor::BuildExecutor;
use crate::build::pandoc::{PandocBuilder, PandocCommandProfile, StandalonePdfLanguage};
use crate::build::pandoc_policy::PandocOutputKind;
use crate::config::MergedConfig;
use crate::constants::{file_names, pandoc};
use crate::doc::templates::{generate_markdown_template, TemplateDocType};
use crate::error::{OmniDocError, Result};
use crate::utils::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMP_INPUT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TemporaryInput {
    path: PathBuf,
}

impl TemporaryInput {
    fn create(path: PathBuf, content: &[u8]) -> Result<Self> {
        fs::write(&path, content)?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TemporaryInput {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn resolve_from(base: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}

fn has_yaml_frontmatter(content: &str) -> bool {
    let trimmed = content
        .trim_start_matches(|character: char| character.is_whitespace() || character == '\u{feff}');
    trimmed.starts_with("---\n") || trimmed.starts_with("---\r\n")
}

fn resolve_pdf_language(
    language: Option<&str>,
    has_frontmatter: bool,
) -> Result<StandalonePdfLanguage> {
    let Some(language) = language.map(str::trim) else {
        return Ok(if has_frontmatter {
            StandalonePdfLanguage::Document
        } else {
            StandalonePdfLanguage::Chinese
        });
    };

    if ["cn", "zh", "zh-cn", "zh_cn"]
        .iter()
        .any(|candidate| language.eq_ignore_ascii_case(candidate))
    {
        return Ok(StandalonePdfLanguage::Chinese);
    }
    if ["en", "en-us", "en_us"]
        .iter()
        .any(|candidate| language.eq_ignore_ascii_case(candidate))
    {
        return Ok(StandalonePdfLanguage::English);
    }

    Err(OmniDocError::UnsupportedLanguage(format!(
        "'{language}'; expected cn or en"
    )))
}

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
        let invocation_dir = std::env::current_dir()?;
        let input = resolve_from(&invocation_dir, input);
        if !input.exists() {
            return Err(OmniDocError::Project(format!(
                "Input file not found: {}",
                input.display()
            )));
        }

        // 确定输出文件路径
        let output_path = if let Some(out) = output {
            resolve_from(&invocation_dir, out)
        } else {
            // 与输入文件同目录，后缀改为 .pdf
            let mut out = input.clone();
            out.set_extension(file_names::PDF_EXTENSION);
            out
        };

        let content = fs::read_to_string(&input).ok();
        let has_frontmatter = content.as_deref().is_some_and(has_yaml_frontmatter);
        let language = resolve_pdf_language(lang, has_frontmatter)?;

        // 如果输入 Markdown 没有 YAML 前言（--- 开头），则基于内置模板生成元数据头，
        // 写入临时文件：元数据头 + 原始内容，然后以该临时文件作为 Pandoc 输入
        let temporary_input = if let Some(content) = content {
            if !has_frontmatter {
                let title = crate::utils::path::file_stem_str(&input).unwrap_or("document");
                let author = self.config.author.as_deref().unwrap_or("Unknown Author");

                // 语言：默认中文（保持与 Python 默认一致）；英文时使用更简洁的头部
                let header = if language == StandalonePdfLanguage::Chinese {
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
                    .as_nanos();
                let sequence = TEMP_INPUT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
                let fname = format!(
                    "omnidoc_md2pdf_{}_{}_{}.md",
                    std::process::id(),
                    ts,
                    sequence
                );
                let mut tmp_path = std::env::temp_dir();
                tmp_path.push(fname);

                Some(TemporaryInput::create(tmp_path, merged.as_bytes())?)
            } else {
                None
            }
        } else {
            None
        };
        let effective_input = temporary_input
            .as_ref()
            .map(TemporaryInput::path)
            .unwrap_or(&input);

        // 构建 Pandoc 选项（可能使用临时合成的输入文件）
        let builder = PandocBuilder::new(self.config.clone())?;
        let project_path = input
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(&invocation_dir);
        let options = builder.build_command_options(
            project_path,
            effective_input,
            &output_path,
            PandocOutputKind::Pdf,
            &PandocCommandProfile::StandalonePdf { language },
        )?;

        // 执行转换
        let args: Vec<&str> = options.iter().map(|s| s.as_str()).collect();
        self.executor
            .execute_in_dir(pandoc::CMD, &args[..], false, Some(project_path))?;

        Ok(())
    }

    /// 将 Markdown 转换为 HTML
    pub fn md_to_html(
        &self,
        input: &Path,
        output: Option<&Path>,
        css: Option<&Path>,
    ) -> Result<()> {
        let invocation_dir = std::env::current_dir()?;
        let input = resolve_from(&invocation_dir, input);
        if !input.exists() {
            return Err(OmniDocError::Project(format!(
                "Input file not found: {}",
                input.display()
            )));
        }

        // 确定输出文件路径
        let output_path = if let Some(out) = output {
            resolve_from(&invocation_dir, out)
        } else {
            // 与输入文件同目录，后缀改为 .html
            let mut out = input.clone();
            out.set_extension(file_names::HTML_EXTENSION);
            out
        };
        let css = css.map(|path| resolve_from(&invocation_dir, path));

        // 构建 Pandoc 选项
        let builder = PandocBuilder::new(self.config.clone())?;
        let project_path = input
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(&invocation_dir);
        let options = builder.build_command_options(
            project_path,
            &input,
            &output_path,
            PandocOutputKind::Html,
            &PandocCommandProfile::StandaloneHtml { css },
        )?;

        // 执行转换
        let args: Vec<&str> = options.iter().map(|s| s.as_str()).collect();
        self.executor
            .execute_in_dir(pandoc::CMD, &args[..], false, Some(project_path))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{has_yaml_frontmatter, resolve_from, resolve_pdf_language, TemporaryInput};
    use crate::build::pandoc::StandalonePdfLanguage;
    use std::fs;
    use std::path::Path;

    #[test]
    fn resolves_nested_relative_paths_from_the_invocation_directory() {
        let base = Path::new("workspace");

        assert_eq!(
            resolve_from(base, Path::new("docs/manual.md")),
            base.join("docs/manual.md")
        );
    }

    #[test]
    fn explicit_pdf_language_is_independent_of_frontmatter() {
        assert_eq!(
            resolve_pdf_language(Some("cn"), true).expect("Chinese language"),
            StandalonePdfLanguage::Chinese
        );
        assert_eq!(
            resolve_pdf_language(Some("en"), true).expect("English language"),
            StandalonePdfLanguage::English
        );
        assert_eq!(
            resolve_pdf_language(None, true).expect("document language"),
            StandalonePdfLanguage::Document
        );
        assert_eq!(
            resolve_pdf_language(None, false).expect("default language"),
            StandalonePdfLanguage::Chinese
        );
    }

    #[test]
    fn recognizes_frontmatter_after_a_utf8_bom() {
        assert!(has_yaml_frontmatter("\u{feff}---\ntitle: Manual\n---\n"));
    }

    #[test]
    fn rejects_unsupported_pdf_languages() {
        let error = resolve_pdf_language(Some("fr"), true).expect_err("unsupported language");
        assert!(error.to_string().contains("expected cn or en"));
    }

    #[test]
    fn temporary_input_is_removed_on_drop() {
        let root = tempfile::tempdir().expect("temporary input root");
        let path = root.path().join("input.md");

        {
            let guard =
                TemporaryInput::create(path.clone(), b"temporary").expect("create temporary input");
            assert_eq!(guard.path(), path);
            assert!(path.is_file());
        }

        assert!(!path.exists());
        assert!(fs::read_dir(root.path())
            .expect("read root")
            .next()
            .is_none());
    }
}
