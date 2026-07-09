use crate::build::executor::BuildExecutor;
use crate::build::pipeline::{BuildPipeline, ProjectType};
use crate::build::source_map::locate_markdown_error;
use crate::config::MergedConfig;
use crate::constants::pandoc;
use crate::error::{OmniDocError, Result};
use crate::utils::fs;
use dirs::data_local_dir;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PandocOutputKind {
    Pdf,
    Html,
    Epub,
    Docx,
    Latex,
}

impl PandocOutputKind {
    fn from_config(config: &MergedConfig) -> Result<Self> {
        let requested = config
            .to
            .as_deref()
            .or(config.pandoc_to_format.as_deref())
            .unwrap_or("pdf")
            .trim()
            .to_ascii_lowercase();

        match requested.as_str() {
            "" | "pdf" => Ok(Self::Pdf),
            "html" | "html4" | "html5" => Ok(Self::Html),
            "epub" | "epub2" | "epub3" => Ok(Self::Epub),
            "docx" => Ok(Self::Docx),
            "latex" | "tex" => Ok(Self::Latex),
            _ => Err(OmniDocError::UnsupportedDocumentType(format!(
                "Unsupported build output format '{}'. Supported formats: pdf, html, epub, docx, latex",
                requested
            ))),
        }
    }

    fn extension(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Html => "html",
            Self::Epub => "epub",
            Self::Docx => "docx",
            Self::Latex => "tex",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Pdf => "PDF",
            Self::Html => "HTML",
            Self::Epub => "EPUB",
            Self::Docx => "DOCX",
            Self::Latex => "LaTeX",
        }
    }

    fn default_to_format(self) -> Option<&'static str> {
        match self {
            Self::Pdf => None,
            Self::Html => Some("html"),
            Self::Epub => Some("epub3"),
            Self::Docx => Some("docx"),
            Self::Latex => Some("latex"),
        }
    }

    fn uses_latex_defaults(self) -> bool {
        matches!(self, Self::Pdf | Self::Latex)
    }

    fn supports_css(self) -> bool {
        matches!(self, Self::Html | Self::Epub)
    }

    fn supports_embed_resources(self) -> bool {
        matches!(self, Self::Html)
    }

    fn supports_standalone(self) -> bool {
        !matches!(self, Self::Docx)
    }
}

/// Pandoc 构建器
/// 实现 markdown 项目的多格式构建功能
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
    fn build_pandoc_options(
        &self,
        entry_file: &Path,
        output_file: &Path,
        output_kind: PandocOutputKind,
    ) -> Result<Vec<String>> {
        let mut options = Vec::new();

        options.push(pandoc::FLAG_FROM.to_string());
        let from_format = self
            .config
            .pandoc_from_format
            .clone()
            .unwrap_or_else(|| pandoc::DEFAULT_FROM_PDF.to_string());
        options.push(from_format);

        if let Some(default_to) = output_kind.default_to_format() {
            options.push(pandoc::FLAG_TO.to_string());
            let to_format = self
                .config
                .pandoc_to_format
                .clone()
                .unwrap_or_else(|| default_to.to_string());
            options.push(to_format);
        }

        let omnidoc_lib = self.get_omnidoc_lib_path();
        self.push_lua_filters(&mut options, output_kind, &omnidoc_lib);

        options.push(pandoc::FLAG_METADATA.to_string());
        let python_path = self
            .config
            .pandoc_python_path
            .clone()
            .unwrap_or_else(|| pandoc::DEFAULT_PYTHON.to_string());
        options.push(format!("pythonPath:{}", python_path));

        options.push(pandoc::FLAG_FILTER.to_string());
        let crossref = self
            .executor
            .check_tool(pandoc::DEFAULT_PLUGIN_CROSSREF)
            .unwrap_or_else(|_| pandoc::DEFAULT_PLUGIN_CROSSREF.to_string());
        options.push(crossref);
        options.push("--citeproc".to_string());

        if output_kind == PandocOutputKind::Pdf {
            options.push(pandoc::FLAG_PDF_ENGINE.to_string());
            let latex_engine = self.executor.check_tool("latex_engine")?;
            options.push(latex_engine);
        }

        options.push(pandoc::FLAG_SYNTAX_HIGHLIGHTING.to_string());
        let syntax_highlighting = self
            .config
            .pandoc_syntax_highlighting
            .clone()
            .unwrap_or_else(|| pandoc::DEFAULT_SYNTAX.to_string());
        options.push(syntax_highlighting);

        options.push(pandoc::FLAG_DATA_DIR.to_string());
        let data_dir = self
            .config
            .pandoc_data_dir
            .clone()
            .unwrap_or_else(|| format!("{}/{}", omnidoc_lib, pandoc::LIB_PANDOC_DATA));
        options.push(data_dir);

        if self.config.pandoc_standalone && output_kind.supports_standalone() {
            options.push(pandoc::FLAG_STANDALONE.to_string());
        }

        if self.config.pandoc_embed_resources && output_kind.supports_embed_resources() {
            options.push(pandoc::FLAG_EMBED_RESOURCES.to_string());
        }

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

        self.push_template(&mut options, output_kind);
        self.push_css(&mut options, output_kind, &omnidoc_lib);
        self.push_format_assets(&mut options, output_kind, &omnidoc_lib);
        self.push_metadata(&mut options, &omnidoc_lib);

        options.extend(self.config.pandoc_options.clone());

        options.push(entry_file.to_string_lossy().to_string());
        options.push(pandoc::FLAG_OUTPUT.to_string());
        options.push(output_file.to_string_lossy().to_string());

        Ok(options)
    }

    fn push_lua_filters(
        &self,
        options: &mut Vec<String>,
        output_kind: PandocOutputKind,
        omnidoc_lib: &str,
    ) {
        let default_filters = if output_kind.uses_latex_defaults() {
            vec![
                "include-files.lua".to_string(),
                "include-code-files.lua".to_string(),
                "diagram-generator.lua".to_string(),
                "ltblr.lua".to_string(),
                "latex-patch.lua".to_string(),
                "fonts-and-alignment.lua".to_string(),
            ]
        } else {
            vec![
                "include-code-files.lua".to_string(),
                "include-files.lua".to_string(),
                "diagram-generator.lua".to_string(),
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
            options.push(format!(
                "{}/{}/{}",
                omnidoc_lib,
                pandoc::LIB_PANDOC_FILTERS,
                filter
            ));
        }
    }

    fn push_template(&self, options: &mut Vec<String>, output_kind: PandocOutputKind) {
        let template = if let Some(template) = &self.config.pandoc_template {
            Some(template.clone())
        } else if output_kind == PandocOutputKind::Pdf {
            Some(pandoc::DEFAULT_TEMPLATE_LATEX.to_string())
        } else {
            None
        };

        if let Some(template) = template {
            options.push(pandoc::FLAG_TEMPLATE.to_string());
            options.push(template);
        }
    }

    fn push_css(
        &self,
        options: &mut Vec<String>,
        output_kind: PandocOutputKind,
        omnidoc_lib: &str,
    ) {
        if !output_kind.supports_css() {
            return;
        }

        let css_path = self
            .config
            .pandoc_css
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(format!(
                    "{}/{}",
                    omnidoc_lib,
                    pandoc::LIB_PANDOC_CSS_DEFAULT
                ))
            });

        if css_path.exists() {
            options.push(pandoc::FLAG_CSS.to_string());
            options.push(css_path.to_string_lossy().to_string());
        }
    }

    fn push_format_assets(
        &self,
        options: &mut Vec<String>,
        output_kind: PandocOutputKind,
        omnidoc_lib: &str,
    ) {
        if output_kind == PandocOutputKind::Docx {
            if let Some(reference_doc) = &self.config.pandoc_reference_doc {
                options.push("--reference-doc".to_string());
                options.push(reference_doc.clone());
            }
        }

        if output_kind == PandocOutputKind::Epub {
            let css_path = self
                .config
                .pandoc_epub_css
                .as_ref()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(format!("{}/pandoc/data/epub.css", omnidoc_lib)));
            if css_path.exists() {
                options.push("--css".to_string());
                options.push(css_path.to_string_lossy().to_string());
            }
        }
    }

    fn push_metadata(&self, options: &mut Vec<String>, omnidoc_lib: &str) {
        if let Some(metadata_file) = &self.config.metadata_file {
            options.push("--metadata-file".to_string());
            options.push(metadata_file.clone());
        }

        if let Some(lang) = &self.config.pandoc_lang {
            if lang != "en" {
                let crossref_yaml = self.config.pandoc_crossref_yaml.clone().unwrap_or_else(|| {
                    format!("{}/{}", omnidoc_lib, pandoc::LIB_PANDOC_CROSSREF_YAML)
                });
                options.push(pandoc::FLAG_META_SHORT.to_string());
                options.push(format!("crossrefYaml={}", crossref_yaml));
            }
        }

        if let Some(author) = &self.config.author {
            options.push(pandoc::FLAG_META_SHORT.to_string());
            options.push(format!("author={}", author));
        }
        if let Some(target) = &self.config.target {
            options.push(pandoc::FLAG_META_SHORT.to_string());
            options.push(format!("title={}", target));
        }

        if self.config.verbose {
            options.push("--verbose".to_string());
        }
    }

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

impl BuildPipeline for PandocBuilder {
    fn build(&self, project_path: &Path, verbose: bool) -> Result<()> {
        let entry_file = if let Some(entry) = &self.config.entry {
            project_path.join(entry)
        } else {
            let main_md = project_path.join("main.md");
            if main_md.exists() {
                main_md
            } else {
                return Err(OmniDocError::Project(
                    "No entry file found. Please specify 'entry' in .omnidoc.toml or create main.md"
                        .to_string(),
                ));
            }
        };

        if !entry_file.exists() {
            return Err(OmniDocError::Project(format!(
                "Entry file not found: {}",
                entry_file.display()
            )));
        }

        let outdir = self
            .config
            .outdir
            .as_ref()
            .map(|s| project_path.join(s))
            .unwrap_or_else(|| project_path.join("build"));

        if !fs::exists(&outdir) {
            fs::create_dir_all(&outdir)?;
        }

        let target_name = self.config.target.as_ref().cloned().unwrap_or_else(|| {
            project_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("document")
                .to_string()
        });

        let output_kind = PandocOutputKind::from_config(&self.config)?;
        let output_file = outdir.join(format!("{}.{}", target_name, output_kind.extension()));
        let options = self.build_pandoc_options(&entry_file, &output_file, output_kind)?;

        let args: Vec<&str> = options.iter().map(|s| s.as_str()).collect();
        if let Err(err) = self.executor.execute(pandoc::CMD, &args[..], verbose) {
            let mut message = err.to_string();
            if let Some(source_hint) = locate_markdown_error(&self.executor, &entry_file, &message)
            {
                message.push_str("\n\n");
                message.push_str(&source_hint);
            }
            return Err(OmniDocError::Project(message));
        }

        if verbose {
            println!("✓ Built {}: {}", output_kind.label(), output_file.display());
        }

        Ok(())
    }

    fn detect_project_type(&self, project_path: &Path) -> Result<ProjectType> {
        if let Some(entry) = &self.config.entry {
            let entry_path = project_path.join(entry);
            if entry_path.exists() {
                return Ok(ProjectType::from_entry_file(&entry_path));
            }
        }

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

#[cfg(test)]
mod tests {
    use super::PandocOutputKind;
    use crate::config::MergedConfig;

    #[test]
    fn defaults_to_pdf_output() {
        let config = MergedConfig::default();

        assert_eq!(
            PandocOutputKind::from_config(&config).expect("output kind"),
            PandocOutputKind::Pdf
        );
    }

    #[test]
    fn accepts_common_output_aliases() {
        let mut config = MergedConfig::default();
        config.to = Some("html5".to_string());
        assert_eq!(
            PandocOutputKind::from_config(&config).expect("output kind"),
            PandocOutputKind::Html
        );

        config.to = Some("tex".to_string());
        assert_eq!(
            PandocOutputKind::from_config(&config).expect("output kind"),
            PandocOutputKind::Latex
        );
    }
}
