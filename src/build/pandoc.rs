use crate::build::executor::BuildExecutor;
use crate::build::pandoc_policy::PandocOutputKind;
use crate::build::pipeline::{BuildPipeline, ProjectType};
use crate::build::source_map::locate_markdown_error;
use crate::config::MergedConfig;
use crate::constants::pandoc;
use crate::error::{OmniDocError, Result};
use crate::utils::fs;
use dirs::data_local_dir;
use std::path::{Path, PathBuf};

/// Pandoc 构建器
/// 实现 markdown 项目的多格式构建功能
pub struct PandocBuilder {
    executor: BuildExecutor,
    config: MergedConfig,
}

#[derive(Debug, Clone, Default)]
pub(crate) enum PandocCommandProfile {
    #[default]
    Project,
    StandalonePdf {
        use_cn: bool,
    },
    StandaloneHtml {
        css: Option<PathBuf>,
    },
}

impl PandocCommandProfile {
    fn default_from_format(&self) -> &'static str {
        match self {
            Self::StandaloneHtml { .. } => pandoc::DEFAULT_FROM_HTML,
            Self::Project | Self::StandalonePdf { .. } => pandoc::DEFAULT_FROM_PDF,
        }
    }

    fn resource_library_dir(&self) -> &'static str {
        match self {
            Self::StandaloneHtml { .. } => pandoc::LIB_PANDOC_HEADERS,
            Self::Project | Self::StandalonePdf { .. } => pandoc::LIB_PANDOC_CSL,
        }
    }
}

impl PandocBuilder {
    pub fn new(config: MergedConfig) -> Result<Self> {
        let executor = BuildExecutor::new(config.tool_paths.clone());
        Ok(Self { executor, config })
    }

    /// 构建 Pandoc 选项
    pub(crate) fn build_command_options(
        &self,
        entry_file: &Path,
        output_file: &Path,
        output_kind: PandocOutputKind,
        profile: &PandocCommandProfile,
    ) -> Result<Vec<String>> {
        let mut options = Vec::new();

        options.push(pandoc::FLAG_FROM.to_string());
        let from_format = self
            .config
            .pandoc_from_format
            .clone()
            .unwrap_or_else(|| profile.default_from_format().to_string());
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
                ".:{}/{}{}",
                omnidoc_lib,
                profile.resource_library_dir(),
                pandoc::RESOURCE_PATH_COMMON_SUFFIX
            )
        };
        options.push(resource_path);

        self.push_template(&mut options, output_kind);
        self.push_css(&mut options, output_kind, &omnidoc_lib, profile);
        self.push_format_assets(&mut options, output_kind, &omnidoc_lib);
        self.push_math_output(&mut options, output_kind);
        self.push_metadata(&mut options, &omnidoc_lib, profile);

        self.push_configured_options(&mut options, output_kind);

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
        for filter in output_kind.filters(&self.config) {
            options.push("--lua-filter".to_string());
            options.push(format!(
                "{}/{}/{}",
                omnidoc_lib,
                pandoc::LIB_PANDOC_FILTERS,
                filter
            ));
        }
    }

    fn push_configured_options(&self, options: &mut Vec<String>, output_kind: PandocOutputKind) {
        output_kind.append_configured_options(&self.config, options);
    }

    fn push_template(&self, options: &mut Vec<String>, output_kind: PandocOutputKind) {
        let template = match output_kind {
            PandocOutputKind::Pdf | PandocOutputKind::Latex => self
                .config
                .pandoc_latex_template
                .clone()
                .or_else(|| self.config.pandoc_template.clone())
                .or_else(|| Some(pandoc::DEFAULT_TEMPLATE_LATEX.to_string())),
            PandocOutputKind::Html => self
                .config
                .pandoc_html_template
                .clone()
                .or_else(|| self.config.pandoc_template.clone()),
            PandocOutputKind::Epub => self
                .config
                .pandoc_epub_template
                .clone()
                .or_else(|| self.config.pandoc_template.clone()),
            PandocOutputKind::Docx => None,
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
        profile: &PandocCommandProfile,
    ) {
        if !matches!(output_kind, PandocOutputKind::Html | PandocOutputKind::Epub) {
            return;
        }

        let base_css = PathBuf::from(omnidoc_lib).join(pandoc::LIB_PANDOC_CSS_BASE);
        if base_css.exists() {
            options.push(pandoc::FLAG_CSS.to_string());
            options.push(base_css.to_string_lossy().to_string());
        }

        if output_kind == PandocOutputKind::Epub {
            return;
        }

        let configured = match profile {
            PandocCommandProfile::StandaloneHtml { css: Some(css) } => {
                Some(css.to_string_lossy().to_string())
            }
            _ => self.config.pandoc_css.clone(),
        };
        let css_path = resolve_css_path(
            configured.as_deref(),
            omnidoc_lib,
            pandoc::LIB_PANDOC_CSS_DEFAULT,
        );

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
            let configured_css = self
                .config
                .pandoc_epub_css
                .as_deref()
                .or(self.config.pandoc_css.as_deref());
            let css_path = resolve_css_path(configured_css, omnidoc_lib, "pandoc/data/epub.css");
            if css_path.exists() {
                options.push("--css".to_string());
                options.push(css_path.to_string_lossy().to_string());
            }
        }
    }

    fn push_math_output(&self, options: &mut Vec<String>, output_kind: PandocOutputKind) {
        if !matches!(output_kind, PandocOutputKind::Html | PandocOutputKind::Epub) {
            return;
        }
        if output_kind.has_explicit_html_math(&self.config) {
            return;
        }
        // MathML is self-contained, works in EPUB 3, and avoids leaving raw
        // TeX delimiters in HTML when Pandoc's plain-HTML math conversion
        // cannot represent a formula.
        options.push("--mathml".to_string());
    }

    fn push_metadata(
        &self,
        options: &mut Vec<String>,
        omnidoc_lib: &str,
        profile: &PandocCommandProfile,
    ) {
        match profile {
            PandocCommandProfile::StandalonePdf { use_cn: true } => {
                self.push_crossref_yaml(options, omnidoc_lib, pandoc::LIB_PANDOC_CROSSREF_YAML);
                return;
            }
            PandocCommandProfile::StandalonePdf { use_cn: false } => return,
            PandocCommandProfile::StandaloneHtml { .. } => {
                self.push_crossref_yaml(
                    options,
                    omnidoc_lib,
                    pandoc::LIB_PANDOC_CROSSREF_YAML_HTML,
                );
                return;
            }
            PandocCommandProfile::Project => {}
        }

        let has_metadata_file = self.config.metadata_file.is_some();
        if let Some(metadata_file) = &self.config.metadata_file {
            options.push("--metadata-file".to_string());
            options.push(metadata_file.clone());
        }

        if let Some(lang) = &self.config.pandoc_lang {
            if lang != "en" {
                self.push_crossref_yaml(options, omnidoc_lib, pandoc::LIB_PANDOC_CROSSREF_YAML);
            }
        }

        // A project metadata file is authoritative for publication metadata.
        // `target` is primarily an output filename and global author defaults
        // must not overwrite a book's explicit title/author.
        if !has_metadata_file {
            if let Some(author) = &self.config.author {
                options.push(pandoc::FLAG_META_SHORT.to_string());
                options.push(format!("author={}", author));
            }
            if let Some(target) = &self.config.target {
                options.push(pandoc::FLAG_META_SHORT.to_string());
                options.push(format!("title={}", target));
            }
        }

        if self.config.verbose {
            options.push("--verbose".to_string());
        }
    }

    fn push_crossref_yaml(&self, options: &mut Vec<String>, omnidoc_lib: &str, fallback: &str) {
        let crossref_yaml = self
            .config
            .pandoc_crossref_yaml
            .clone()
            .unwrap_or_else(|| format!("{}/{}", omnidoc_lib, fallback));
        options.push(pandoc::FLAG_META_SHORT.to_string());
        options.push(format!("crossrefYaml={}", crossref_yaml));
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

fn resolve_css_path(configured: Option<&str>, omnidoc_lib: &str, fallback: &str) -> PathBuf {
    let Some(configured) = configured else {
        return PathBuf::from(omnidoc_lib).join(fallback);
    };
    let project_path = PathBuf::from(configured);
    if project_path.exists() {
        return project_path;
    }
    let shared_path = PathBuf::from(omnidoc_lib)
        .join("pandoc/css")
        .join(configured);
    if shared_path.exists() {
        return shared_path;
    }
    project_path
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
        let options = self.build_command_options(
            &entry_file,
            &output_file,
            output_kind,
            &PandocCommandProfile::Project,
        )?;

        let args: Vec<&str> = options.iter().map(|s| s.as_str()).collect();
        if let Err(err) =
            self.executor
                .execute_in_dir(pandoc::CMD, &args[..], verbose, Some(project_path))
        {
            let mut message = err.to_string();
            if let Some(source_hint) =
                locate_markdown_error(&self.executor, project_path, &entry_file, &message)
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
    use super::resolve_css_path;
    use crate::build::pandoc::{PandocBuilder, PandocCommandProfile};
    use crate::build::pandoc_policy::PandocOutputKind;
    use crate::config::MergedConfig;
    use std::collections::BTreeMap;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

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
        let mut config = MergedConfig {
            to: Some("html5".to_string()),
            ..Default::default()
        };
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

    #[test]
    fn appends_only_the_selected_formats_options_after_common_options() {
        let builder = PandocBuilder::new(MergedConfig {
            pandoc_options: vec!["--toc-depth=1".to_string()],
            pandoc_format_options: BTreeMap::from([
                ("epub".to_string(), vec!["--toc-depth=3".to_string()]),
                ("pdf".to_string(), vec!["--pdf-option".to_string()]),
            ]),
            ..Default::default()
        })
        .expect("pandoc builder");
        let mut options = Vec::new();

        builder.push_configured_options(&mut options, PandocOutputKind::Epub);

        assert_eq!(options, vec!["--toc-depth=1", "--toc-depth=3"]);
        assert!(!options.iter().any(|option| option == "--pdf-option"));
    }

    #[test]
    fn uses_format_specific_templates_without_docx_template_flag() {
        let html_config = MergedConfig {
            pandoc_html_template: Some("html-template.html".to_string()),
            ..Default::default()
        };
        let html_builder = PandocBuilder::new(html_config).expect("html builder");
        let mut html_options = Vec::new();
        html_builder.push_template(&mut html_options, PandocOutputKind::Html);
        assert_eq!(
            html_options,
            vec!["--template".to_string(), "html-template.html".to_string()]
        );

        let epub_config = MergedConfig {
            pandoc_template: Some("generic-template.html".to_string()),
            pandoc_epub_template: Some("epub-template.html".to_string()),
            ..Default::default()
        };
        let epub_builder = PandocBuilder::new(epub_config).expect("epub builder");
        let mut epub_options = Vec::new();
        epub_builder.push_template(&mut epub_options, PandocOutputKind::Epub);
        assert_eq!(
            epub_options,
            vec!["--template".to_string(), "epub-template.html".to_string()]
        );

        let docx_config = MergedConfig {
            pandoc_template: Some("generic-template.html".to_string()),
            ..Default::default()
        };
        let docx_builder = PandocBuilder::new(docx_config).expect("docx builder");
        let mut docx_options = Vec::new();
        docx_builder.push_template(&mut docx_options, PandocOutputKind::Docx);
        assert!(docx_options.is_empty());
    }

    #[test]
    fn resolves_named_css_from_omnidoc_libs_and_avoids_epub_duplicates() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let library = std::env::temp_dir().join(format!("omnidoc-css-{nonce}"));
        let css_dir = library.join("pandoc/css");
        fs::create_dir_all(&css_dir).expect("css dir");
        let base_css = css_dir.join("omnidoc-base.css");
        fs::write(&base_css, ".omni-display-math { text-align: center; }\n").expect("base css");
        let shared_css = css_dir.join("engineering-book.css");
        fs::write(&shared_css, "body { max-width: 56rem; }\n").expect("shared css");

        assert_eq!(
            resolve_css_path(
                Some("engineering-book.css"),
                library.to_str().expect("library path"),
                "pandoc/data/epub.css",
            ),
            shared_css
        );

        let builder = PandocBuilder::new(MergedConfig {
            pandoc_css: Some("engineering-book.css".to_string()),
            pandoc_epub_css: Some("engineering-book.css".to_string()),
            ..Default::default()
        })
        .expect("pandoc builder");
        let mut options = Vec::new();
        builder.push_css(
            &mut options,
            PandocOutputKind::Epub,
            library.to_str().expect("library path"),
            &PandocCommandProfile::Project,
        );
        builder.push_format_assets(
            &mut options,
            PandocOutputKind::Epub,
            library.to_str().expect("library path"),
        );
        assert_eq!(
            options,
            vec![
                "--css".to_string(),
                base_css.to_string_lossy().to_string(),
                "--css".to_string(),
                shared_css.to_string_lossy().to_string()
            ]
        );

        fs::remove_dir_all(library).expect("cleanup");
    }

    #[test]
    fn metadata_file_is_not_overridden_by_target_or_global_author() {
        let config = MergedConfig {
            metadata_file: Some("book-metadata.yaml".to_string()),
            target: Some("output-filename".to_string()),
            author: Some("global default author".to_string()),
            ..Default::default()
        };
        let builder = PandocBuilder::new(config).expect("pandoc builder");
        let mut options = Vec::new();

        builder.push_metadata(
            &mut options,
            "/tmp/omnidoc-libs",
            &PandocCommandProfile::Project,
        );

        assert!(options.windows(2).any(|pair| pair
            == [
                "--metadata-file".to_string(),
                "book-metadata.yaml".to_string()
            ]));
        assert!(!options
            .iter()
            .any(|option| option == "title=output-filename"));
        assert!(!options
            .iter()
            .any(|option| option == "author=global default author"));
    }

    #[test]
    fn html_and_epub_default_to_mathml_without_overriding_explicit_math_output() {
        let builder = PandocBuilder::new(MergedConfig::default()).expect("pandoc builder");
        let mut html_options = Vec::new();
        builder.push_math_output(&mut html_options, PandocOutputKind::Html);
        assert_eq!(html_options, vec!["--mathml"]);

        let mut epub_options = Vec::new();
        builder.push_math_output(&mut epub_options, PandocOutputKind::Epub);
        assert_eq!(epub_options, vec!["--mathml"]);

        let explicit = PandocBuilder::new(MergedConfig {
            pandoc_options: vec!["--mathjax=https://cdn.example/mathjax.js".to_string()],
            ..Default::default()
        })
        .expect("pandoc builder");
        let mut explicit_options = Vec::new();
        explicit.push_math_output(&mut explicit_options, PandocOutputKind::Html);
        assert!(explicit_options.is_empty());
    }

    #[test]
    fn standalone_html_uses_the_shared_command_builder_profile() {
        let root = tempfile::tempdir().expect("tempdir");
        let css = root.path().join("standalone.css");
        fs::write(&css, "body { color: navy; }\n").expect("css");
        let builder = PandocBuilder::new(MergedConfig {
            lib_path: Some(root.path().to_string_lossy().to_string()),
            pandoc_format_options: BTreeMap::from([(
                "html".to_string(),
                vec!["--mathjax".to_string(), "--toc-depth=3".to_string()],
            )]),
            ..Default::default()
        })
        .expect("builder");

        let options = builder
            .build_command_options(
                std::path::Path::new("input.md"),
                std::path::Path::new("output.html"),
                PandocOutputKind::Html,
                &PandocCommandProfile::StandaloneHtml {
                    css: Some(css.clone()),
                },
            )
            .expect("options");

        assert!(options.windows(2).any(|pair| pair == ["-f", "markdown"]));
        assert!(options.windows(2).any(|pair| pair == ["-t", "html"]));
        assert!(options
            .windows(2)
            .any(|pair| pair == ["--css", css.to_string_lossy().as_ref()]));
        assert!(options.iter().any(|option| option == "--mathjax"));
        assert!(!options.iter().any(|option| option == "--mathml"));
        assert!(options
            .iter()
            .any(|option| option.contains("pandoc/headers")));
        assert!(options
            .iter()
            .any(|option| option.contains("pandoc/data/crossref.yaml")));
    }

    #[test]
    fn standalone_pdf_profile_controls_crossref_metadata() {
        let builder = PandocBuilder::new(MergedConfig::default()).expect("builder");
        let mut chinese = Vec::new();
        builder.push_metadata(
            &mut chinese,
            "/tmp/omnidoc-libs",
            &PandocCommandProfile::StandalonePdf { use_cn: true },
        );
        assert!(chinese
            .iter()
            .any(|option| option.contains("pandoc/crossref.yaml")));

        let mut english = Vec::new();
        builder.push_metadata(
            &mut english,
            "/tmp/omnidoc-libs",
            &PandocCommandProfile::StandalonePdf { use_cn: false },
        );
        assert!(english.is_empty());
    }
}
