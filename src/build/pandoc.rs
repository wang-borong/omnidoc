use crate::build::executor::BuildExecutor;
use crate::build::pandoc_policy::PandocOutputKind;
use crate::build::pipeline::{BuildPipeline, ProjectType};
use crate::build::source_map::locate_markdown_error;
use crate::cli::handlers::theme::{load_theme_manifest, ThemeManifest};
use crate::config::MergedConfig;
use crate::constants::pandoc;
use crate::error::{OmniDocError, Result};
use crate::latex_recorder;
use crate::project_tools::{
    filter_depfile_metadata_key, filter_depfile_name, INCLUDE_CODE_DEPFILE, INCLUDE_DEPFILE,
    LATEX_INPUT_DEPFILE,
};
use crate::utils::directories::data_local_dir;
use crate::utils::fs;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Pandoc 构建器
/// 实现 markdown 项目的多格式构建功能
pub struct PandocBuilder {
    executor: BuildExecutor,
    config: MergedConfig,
    theme: Option<ThemeManifest>,
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
        let theme = load_selected_theme(&config)?;
        Ok(Self {
            executor,
            config,
            theme,
        })
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

        // Diagram filters may need to invoke OmniDoc's native renderers (for
        // example fenced `bitfield` blocks). Pass the exact running binary so
        // builds do not accidentally resolve an older installation from PATH.
        if let Ok(omnidoc_path) = std::env::current_exe() {
            options.push(pandoc::FLAG_METADATA.to_string());
            options.push(format!("omnidocPath:{}", omnidoc_path.display()));
        }

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
        // External filters such as pandoc-crossref may rewrite Code and
        // CodeBlock nodes to raw LaTeX before Pandoc decides which template
        // feature variables are needed. Preserve the writer contract for the
        // idiomatic LaTeX backend so Pandoc's own default template still loads
        // listings and defines \passthrough.
        if matches!(output_kind, PandocOutputKind::Pdf | PandocOutputKind::Latex)
            && self
                .config
                .pandoc_syntax_highlighting
                .as_deref()
                .unwrap_or(pandoc::DEFAULT_SYNTAX)
                == "idiomatic"
        {
            options.push("--variable".to_string());
            options.push("listings=true".to_string());
        }

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

        if self.config.pandoc_toc {
            options.push("--toc".to_string());
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

        self.push_template(&mut options, output_kind, &omnidoc_lib);
        self.push_default_latex_headers(&mut options, output_kind, &omnidoc_lib);
        self.push_theme_latex_headers(&mut options, output_kind, &omnidoc_lib);
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
        let mut added = std::collections::BTreeSet::new();
        for filter in output_kind.filters(&self.config) {
            options.push("--lua-filter".to_string());
            let path = format!("{}/{}/{}", omnidoc_lib, pandoc::LIB_PANDOC_FILTERS, filter);
            added.insert(PathBuf::from(&path));
            options.push(path);
        }
        if let Some(theme) = &self.theme {
            for relative in &theme.resources.lua_filters {
                let path = PathBuf::from(omnidoc_lib).join(relative);
                if added.insert(path.clone()) {
                    options.push("--lua-filter".to_string());
                    options.push(path.to_string_lossy().to_string());
                }
            }
        }
    }

    fn push_configured_options(&self, options: &mut Vec<String>, output_kind: PandocOutputKind) {
        output_kind.append_configured_options(&self.config, options);
    }

    fn push_theme_latex_headers(
        &self,
        options: &mut Vec<String>,
        output_kind: PandocOutputKind,
        omnidoc_lib: &str,
    ) {
        if !output_kind.uses_latex_defaults() {
            return;
        }
        if let Some(theme) = &self.theme {
            for relative in &theme.resources.latex_headers {
                options.push("--include-in-header".to_string());
                options.push(
                    PathBuf::from(omnidoc_lib)
                        .join(relative)
                        .to_string_lossy()
                        .to_string(),
                );
            }
        }
    }

    fn push_default_latex_headers(
        &self,
        options: &mut Vec<String>,
        output_kind: PandocOutputKind,
        omnidoc_lib: &str,
    ) {
        if !output_kind.uses_latex_defaults()
            || !output_kind.filters(&self.config).contains(&"emoji.lua")
        {
            return;
        }
        options.push("--include-in-header".to_string());
        options.push(
            PathBuf::from(omnidoc_lib)
                .join(pandoc::LIB_PANDOC_HEADER_EMOJI)
                .to_string_lossy()
                .to_string(),
        );
    }

    fn push_template(
        &self,
        options: &mut Vec<String>,
        output_kind: PandocOutputKind,
        omnidoc_lib: &str,
    ) {
        let theme_template = self.theme.as_ref().and_then(|theme| match output_kind {
            PandocOutputKind::Pdf | PandocOutputKind::Latex => {
                theme.resources.latex_template.as_ref()
            }
            PandocOutputKind::Html => theme.resources.html_template.as_ref(),
            PandocOutputKind::Epub => theme.resources.epub_template.as_ref(),
            PandocOutputKind::Docx | PandocOutputKind::Pptx => None,
        });
        let theme_template = theme_template.map(|relative| {
            PathBuf::from(omnidoc_lib)
                .join(relative)
                .to_string_lossy()
                .to_string()
        });
        let template = match output_kind {
            PandocOutputKind::Pdf | PandocOutputKind::Latex => self
                .config
                .pandoc_latex_template
                .clone()
                .or_else(|| self.config.pandoc_template.clone())
                .or_else(|| theme_template.clone()),
            PandocOutputKind::Html => self
                .config
                .pandoc_html_template
                .clone()
                .or_else(|| self.config.pandoc_template.clone())
                .or_else(|| theme_template.clone()),
            PandocOutputKind::Epub => self
                .config
                .pandoc_epub_template
                .clone()
                .or_else(|| self.config.pandoc_template.clone())
                .or(theme_template),
            PandocOutputKind::Docx | PandocOutputKind::Pptx => None,
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

        let base_css = join_portable_relative(omnidoc_lib, pandoc::LIB_PANDOC_CSS_BASE);
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
            _ => self
                .config
                .pandoc_css
                .clone()
                .or_else(|| self.theme_css(output_kind)),
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

        if output_kind == PandocOutputKind::Pptx {
            let configured = self
                .config
                .pandoc_pptx_reference_doc
                .as_ref()
                .or(self.config.pandoc_reference_doc.as_ref());
            let reference_doc = configured.cloned().or_else(|| {
                self.theme
                    .as_ref()
                    .and_then(|theme| theme.resources.pptx_reference_doc.as_ref())
                    .map(|relative| {
                        PathBuf::from(omnidoc_lib)
                            .join(relative)
                            .to_string_lossy()
                            .to_string()
                    })
            });
            if let Some(reference_doc) = reference_doc {
                options.push("--reference-doc".to_string());
                options.push(reference_doc);
            }
        }

        if output_kind == PandocOutputKind::Epub {
            let configured_css = self
                .config
                .pandoc_epub_css
                .as_deref()
                .or(self.config.pandoc_css.as_deref())
                .map(str::to_string)
                .or_else(|| self.theme_css(output_kind));
            let css_path = resolve_css_path(
                configured_css.as_deref(),
                omnidoc_lib,
                "pandoc/data/epub.css",
            );
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

        if !has_metadata_file {
            self.push_theme_metadata_defaults(options);
        }

        if let Some(lang) = self.effective_lang() {
            if lang != "en" {
                self.push_crossref_yaml(options, omnidoc_lib, pandoc::LIB_PANDOC_CROSSREF_YAML);
            }
        }

        if let Some(lang) = self.config.pandoc_lang.as_deref() {
            options.push(pandoc::FLAG_META_SHORT.to_string());
            options.push(format!("lang={lang}"));
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

    fn push_theme_metadata_defaults(&self, options: &mut Vec<String>) {
        let Some(theme) = self.theme.as_ref() else {
            return;
        };
        for (key, value) in &theme.metadata.defaults {
            if key == "lang" && self.config.pandoc_lang.is_some() {
                continue;
            }
            options.push(pandoc::FLAG_META_SHORT.to_string());
            options.push(format!("{key}={value}"));
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

    fn theme_css(&self, output_kind: PandocOutputKind) -> Option<String> {
        let theme = self.theme.as_ref()?;
        match output_kind {
            PandocOutputKind::Html => theme.resources.html_css.first().cloned(),
            PandocOutputKind::Epub => theme.resources.epub_css.first().cloned(),
            _ => None,
        }
    }

    fn effective_lang(&self) -> Option<&str> {
        self.config.pandoc_lang.as_deref().or_else(|| {
            self.theme
                .as_ref()
                .and_then(|theme| theme.metadata.defaults.get("lang"))
                .map(String::as_str)
        })
    }
}

pub(crate) fn load_selected_theme(config: &MergedConfig) -> Result<Option<ThemeManifest>> {
    let Some(name) = config.theme_name.as_deref() else {
        return Ok(None);
    };
    let library = config
        .lib_path
        .as_ref()
        .map(PathBuf::from)
        .or_else(|| data_local_dir().map(|path| path.join("omnidoc")))
        .ok_or_else(|| OmniDocError::Config("OmniDoc library path is unavailable".to_string()))?;
    let theme = load_theme_manifest(&library, name)?;
    if let Some(requested) = config.theme_version.as_deref() {
        let requirement = semver::VersionReq::parse(requested).map_err(|error| {
            OmniDocError::Config(format!(
                "Invalid theme.version requirement '{}': {}",
                requested, error
            ))
        })?;
        let installed = semver::Version::parse(&theme.version).map_err(|error| {
            OmniDocError::Config(format!(
                "Invalid installed theme version '{}': {}",
                theme.version, error
            ))
        })?;
        if !requirement.matches(&installed) {
            return Err(OmniDocError::Config(format!(
                "Theme '{}' version {} does not satisfy {}",
                name, installed, requested
            )));
        }
    }
    if let Some(requested) = config.theme_compatibility.as_deref() {
        if theme.compatibility.as_deref() != Some(requested) {
            return Err(OmniDocError::Config(format!(
                "Theme '{}' compatibility '{}' does not match requested '{}'",
                name,
                theme.compatibility.as_deref().unwrap_or("default"),
                requested
            )));
        }
    }
    Ok(Some(theme))
}

fn resolve_css_path(configured: Option<&str>, omnidoc_lib: &str, fallback: &str) -> PathBuf {
    let Some(configured) = configured else {
        return join_portable_relative(omnidoc_lib, fallback);
    };
    let project_path = PathBuf::from(configured);
    if project_path.exists() {
        return project_path;
    }
    let shared_path = join_portable_relative(omnidoc_lib, "pandoc/css").join(configured);
    if shared_path.exists() {
        return shared_path;
    }
    let bundle_path = PathBuf::from(omnidoc_lib).join(configured);
    if bundle_path.exists() {
        return bundle_path;
    }
    project_path
}

fn join_portable_relative(root: &str, relative: &str) -> PathBuf {
    relative
        .split(['/', '\\'])
        .filter(|component| !component.is_empty())
        .fold(PathBuf::from(root), |path, component| path.join(component))
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
        let mut options = self.build_command_options(
            &entry_file,
            &output_file,
            output_kind,
            &PandocCommandProfile::Project,
        )?;
        let cache_dir = project_path.join(".omnidoc-cache");
        fs::create_dir_all(&cache_dir)?;
        for (key, file) in [
            ("omnidoc-include-depfile", INCLUDE_DEPFILE),
            ("omnidoc-include-code-depfile", INCLUDE_CODE_DEPFILE),
        ] {
            options.push(pandoc::FLAG_METADATA.to_string());
            options.push(format!("{}={}", key, cache_dir.join(file).display()));
        }
        let mut generic_depfiles = BTreeSet::new();
        for filter in output_kind.filters(&self.config) {
            let Some(key) = filter_depfile_metadata_key(filter) else {
                continue;
            };
            let Some(file) = filter_depfile_name(filter) else {
                continue;
            };
            if generic_depfiles.insert(key.clone()) {
                options.push(pandoc::FLAG_METADATA.to_string());
                options.push(format!("{}={}", key, cache_dir.join(file).display()));
            }
        }

        let mut recorder_environment = Vec::new();
        if output_kind == PandocOutputKind::Pdf {
            let depfile = cache_dir.join(LATEX_INPUT_DEPFILE);
            let real_engine = self.executor.check_tool("latex_engine")?;
            match latex_recorder::prepare_wrapper(project_path, Path::new(&real_engine), &depfile)?
            {
                Some(recorder) => {
                    if let Some(index) = options
                        .iter()
                        .position(|option| option == pandoc::FLAG_PDF_ENGINE)
                    {
                        if let Some(engine) = options.get_mut(index + 1) {
                            *engine = recorder.wrapper.to_string_lossy().to_string();
                        }
                    }
                    options.push("--pdf-engine-opt=-recorder".to_string());
                    recorder_environment = recorder.environment;
                }
                None => {
                    if depfile.exists() {
                        fs::remove_file(&depfile)?;
                    }
                }
            }
        }

        let args: Vec<&str> = options.iter().map(|s| s.as_str()).collect();
        if let Err(err) = self.executor.execute_in_dir_with_env(
            pandoc::CMD,
            &args[..],
            verbose,
            Some(project_path),
            &recorder_environment,
        ) {
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
    fn adds_emoji_header_only_when_the_latex_emoji_filter_is_active() {
        let builder = PandocBuilder::new(MergedConfig::default()).expect("pandoc builder");
        let mut pdf_options = Vec::new();
        builder.push_default_latex_headers(&mut pdf_options, PandocOutputKind::Pdf, "/tmp/omnidoc");
        assert_eq!(
            pdf_options,
            vec![
                "--include-in-header".to_string(),
                std::path::PathBuf::from("/tmp/omnidoc")
                    .join("pandoc/headers/emoji.tex")
                    .to_string_lossy()
                    .to_string(),
            ]
        );

        let mut html_options = Vec::new();
        builder.push_default_latex_headers(
            &mut html_options,
            PandocOutputKind::Html,
            "/tmp/omnidoc",
        );
        assert!(html_options.is_empty());

        let custom = PandocBuilder::new(MergedConfig {
            pandoc_lua_filters: vec!["custom.lua".to_string()],
            ..Default::default()
        })
        .expect("custom pandoc builder");
        let mut custom_options = Vec::new();
        custom.push_default_latex_headers(
            &mut custom_options,
            PandocOutputKind::Pdf,
            "/tmp/omnidoc",
        );
        assert!(custom_options.is_empty());
    }

    #[test]
    fn enables_table_of_contents_from_pandoc_config() {
        let root = tempfile::tempdir().expect("tempdir");
        let builder = PandocBuilder::new(MergedConfig {
            lib_path: Some(root.path().to_string_lossy().to_string()),
            pandoc_toc: true,
            ..Default::default()
        })
        .expect("pandoc builder");

        let options = builder
            .build_command_options(
                std::path::Path::new("input.md"),
                std::path::Path::new("output.html"),
                PandocOutputKind::Html,
                &PandocCommandProfile::Project,
            )
            .expect("html options");

        assert!(options.iter().any(|option| option == "--toc"));
        assert!(options
            .iter()
            .any(|option| option.starts_with("omnidocPath:")));
    }

    #[test]
    fn uses_format_specific_templates_without_office_template_flags() {
        let html_config = MergedConfig {
            pandoc_html_template: Some("html-template.html".to_string()),
            ..Default::default()
        };
        let html_builder = PandocBuilder::new(html_config).expect("html builder");
        let mut html_options = Vec::new();
        html_builder.push_template(&mut html_options, PandocOutputKind::Html, "/tmp/omnidoc");
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
        epub_builder.push_template(&mut epub_options, PandocOutputKind::Epub, "/tmp/omnidoc");
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
        docx_builder.push_template(&mut docx_options, PandocOutputKind::Docx, "/tmp/omnidoc");
        assert!(docx_options.is_empty());

        let mut pptx_options = Vec::new();
        docx_builder.push_template(&mut pptx_options, PandocOutputKind::Pptx, "/tmp/omnidoc");
        assert!(pptx_options.is_empty());
    }

    #[test]
    fn resolves_named_css_from_omnidoc_libs_and_avoids_epub_duplicates() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let library = std::env::temp_dir().join(format!("omnidoc-css-{nonce}"));
        let css_dir = library.join("pandoc").join("css");
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
    fn selected_theme_supplies_css_and_latex_header() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let library = std::env::temp_dir().join(format!("omnidoc-theme-build-{nonce}"));
        fs::create_dir_all(library.join("themes")).expect("theme dir");
        fs::create_dir_all(library.join("pandoc/css")).expect("css dir");
        fs::create_dir_all(library.join("pandoc/headers")).expect("headers dir");
        fs::create_dir_all(library.join("pandoc/data/templates")).expect("templates dir");
        fs::create_dir_all(library.join("pandoc/data/reference-docs")).expect("reference docs dir");
        let css = library.join("pandoc/css/engineering-book.css");
        let header = library.join("pandoc/headers/engineering-book.tex");
        let template = library.join("pandoc/data/templates/engineering-book.latex");
        let slides = library.join("pandoc/data/reference-docs/engineering-slides.pptx");
        fs::write(&css, "body { max-width: 56rem; }\n").expect("theme css");
        fs::write(&header, "\\usepackage{omni-engineering-book}\n").expect("theme header");
        fs::write(&template, "$body$\n").expect("theme template");
        fs::write(&slides, "pptx reference").expect("pptx reference");
        fs::write(
            library.join("themes/engineering-book.toml"),
            r#"manifest_version = 1
name = "engineering-book"
version = "1.0.0"
compatible_omnidoc = ">=1.3.0,<2.0.0"
compatibility = "readium"

[resources]
html_css = ["pandoc/css/engineering-book.css"]
latex_headers = ["pandoc/headers/engineering-book.tex"]
latex_template = "pandoc/data/templates/engineering-book.latex"
pptx_reference_doc = "pandoc/data/reference-docs/engineering-slides.pptx"

[metadata.defaults]
lang = "zh-CN"
documentclass = "scrbook"
"#,
        )
        .expect("theme manifest");

        let builder = PandocBuilder::new(MergedConfig {
            lib_path: Some(library.to_string_lossy().to_string()),
            theme_name: Some("engineering-book".to_string()),
            theme_version: Some("1".to_string()),
            theme_compatibility: Some("readium".to_string()),
            ..Default::default()
        })
        .expect("themed builder");
        let mut html_options = Vec::new();
        builder.push_css(
            &mut html_options,
            PandocOutputKind::Html,
            library.to_str().expect("library path"),
            &PandocCommandProfile::Project,
        );
        assert_eq!(
            html_options,
            vec!["--css".to_string(), css.to_string_lossy().to_string()]
        );

        let mut latex_options = Vec::new();
        builder.push_theme_latex_headers(
            &mut latex_options,
            PandocOutputKind::Pdf,
            library.to_str().expect("library path"),
        );
        assert_eq!(
            latex_options,
            vec![
                "--include-in-header".to_string(),
                header.to_string_lossy().to_string()
            ]
        );

        let mut pptx_options = Vec::new();
        builder.push_format_assets(
            &mut pptx_options,
            PandocOutputKind::Pptx,
            library.to_str().expect("library path"),
        );
        assert_eq!(
            pptx_options,
            vec![
                "--reference-doc".to_string(),
                slides.to_string_lossy().to_string()
            ]
        );
        let mut template_options = Vec::new();
        builder.push_template(
            &mut template_options,
            PandocOutputKind::Pdf,
            library.to_str().expect("library path"),
        );
        assert_eq!(
            template_options,
            vec![
                "--template".to_string(),
                template.to_string_lossy().to_string()
            ]
        );
        let explicit_template = PandocBuilder::new(MergedConfig {
            lib_path: Some(library.to_string_lossy().to_string()),
            theme_name: Some("engineering-book".to_string()),
            pandoc_latex_template: Some("project-template.latex".to_string()),
            ..Default::default()
        })
        .expect("explicit template builder");
        let mut explicit_template_options = Vec::new();
        explicit_template.push_template(
            &mut explicit_template_options,
            PandocOutputKind::Pdf,
            library.to_str().expect("library path"),
        );
        assert_eq!(
            explicit_template_options,
            vec![
                "--template".to_string(),
                "project-template.latex".to_string()
            ]
        );

        let mut metadata_options = Vec::new();
        builder.push_metadata(
            &mut metadata_options,
            library.to_str().expect("library path"),
            &PandocCommandProfile::Project,
        );
        assert!(metadata_options
            .windows(2)
            .any(|pair| pair == ["-M".to_string(), "lang=zh-CN".to_string()]));
        assert!(metadata_options
            .windows(2)
            .any(|pair| pair == ["-M".to_string(), "documentclass=scrbook".to_string()]));

        let explicit_lang = PandocBuilder::new(MergedConfig {
            lib_path: Some(library.to_string_lossy().to_string()),
            theme_name: Some("engineering-book".to_string()),
            pandoc_lang: Some("en".to_string()),
            ..Default::default()
        })
        .expect("explicit language builder");
        let mut explicit_options = Vec::new();
        explicit_lang.push_metadata(
            &mut explicit_options,
            library.to_str().expect("library path"),
            &PandocCommandProfile::Project,
        );
        assert!(explicit_options
            .windows(2)
            .any(|pair| pair == ["-M".to_string(), "lang=en".to_string()]));
        assert!(!explicit_options.iter().any(|option| option == "lang=zh-CN"));

        let metadata_file = PandocBuilder::new(MergedConfig {
            lib_path: Some(library.to_string_lossy().to_string()),
            theme_name: Some("engineering-book".to_string()),
            metadata_file: Some("book.yaml".to_string()),
            ..Default::default()
        })
        .expect("metadata file builder");
        let mut file_options = Vec::new();
        metadata_file.push_metadata(
            &mut file_options,
            library.to_str().expect("library path"),
            &PandocCommandProfile::Project,
        );
        assert!(!file_options
            .iter()
            .any(|option| option == "documentclass=scrbook"));

        fs::remove_dir_all(library).expect("cleanup");
    }

    #[test]
    fn latex_uses_pandoc_default_template_and_preserves_idiomatic_listings() {
        let root = tempfile::tempdir().expect("tempdir");
        let builder = PandocBuilder::new(MergedConfig {
            lib_path: Some(root.path().to_string_lossy().to_string()),
            pandoc_standalone: true,
            ..Default::default()
        })
        .expect("pandoc builder");

        let options = builder
            .build_command_options(
                std::path::Path::new("input.md"),
                std::path::Path::new("output.tex"),
                PandocOutputKind::Latex,
                &PandocCommandProfile::Project,
            )
            .expect("latex options");

        assert!(!options.iter().any(|option| option == "--template"));
        assert!(options
            .windows(2)
            .any(|pair| pair == ["--variable".to_string(), "listings=true".to_string()]));
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
