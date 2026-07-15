use crate::build::pandoc::load_selected_theme;
use crate::build::pandoc_policy::{is_supported_format_key, PandocOutputKind};
use crate::cli::handlers::theme::font_family_matches;
use crate::config::MergedConfig;
use crate::constants::pandoc;
use crate::epub::{is_supported_epub_profile, EpubCompatibilityReport};
use crate::error::{OmniDocError, Result};
use blake3::Hasher;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;

const CACHE_DIR: &str = ".omnidoc-cache";
pub(crate) const INCLUDE_DEPFILE: &str = "include-files.d";
pub(crate) const INCLUDE_CODE_DEPFILE: &str = "include-code-files.d";
const LOCK_FILE: &str = "omnidoc.lock";
const REPORT_FILE: &str = "omnidoc-report.json";
const CACHE_VERSION: u32 = 4;
const LOCK_VERSION: u32 = 4;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum IssueSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectIssue {
    pub severity: IssueSeverity,
    pub message: String,
    pub path: Option<String>,
    pub line: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyGraph {
    pub files: Vec<String>,
    pub resources: Vec<ResolvedResource>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResolvedResource {
    pub logical_name: String,
    pub resolved_from: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildReport {
    pub output: String,
    pub target: String,
    pub skipped: bool,
    pub cache_reason: String,
    pub duration_ms: u64,
    pub input_digest: String,
    pub artifact_digest: Option<String>,
    pub compatibility: Option<EpubCompatibilityReport>,
    pub dependencies: Vec<String>,
    pub resources: Vec<LockedResource>,
    pub toolchain: BTreeMap<String, String>,
    pub issues: Vec<ProjectIssue>,
    pub timestamp_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildReportDocument {
    pub omnidoc_version: String,
    pub generated_at_unix: u64,
    pub reports: Vec<BuildReport>,
}

pub struct BuildReportContext<'a> {
    pub output: String,
    pub target: String,
    pub skipped: bool,
    pub cache_reason: String,
    pub duration_ms: u64,
    pub input_digest: String,
    pub graph: &'a DependencyGraph,
    pub config: &'a MergedConfig,
    pub artifact: &'a Path,
    pub compatibility: Option<EpubCompatibilityReport>,
    pub issues: Vec<ProjectIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BuildCache {
    cache_version: u32,
    input_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockFile {
    pub lock_version: u32,
    pub omnidoc_version: String,
    pub library: Option<LockedLibrary>,
    pub toolchain: BTreeMap<String, String>,
    pub targets: BTreeMap<String, LockedTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LockedTarget {
    pub input_digest: String,
    pub resources: Vec<LockedResource>,
    pub dependencies: Vec<String>,
}

pub struct LockTargetInput<'a> {
    pub output: &'a str,
    pub config: &'a MergedConfig,
    pub graph: &'a DependencyGraph,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LockedLibrary {
    pub version: Option<String>,
    pub revision: Option<String>,
    pub manifest_digest: Option<String>,
    pub checksums_digest: Option<String>,
    pub digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LockedResource {
    pub logical_name: String,
    pub resolved_from: String,
    pub digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockStatus {
    pub exists: bool,
    pub up_to_date: bool,
    pub library_up_to_date: bool,
    pub toolchain_up_to_date: bool,
    pub missing_targets: Vec<String>,
    pub extra_targets: Vec<String>,
    pub targets: BTreeMap<String, LockTargetStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockTargetStatus {
    pub up_to_date: bool,
    pub expected_digest: String,
    pub actual_digest: Option<String>,
    pub missing_dependencies: Vec<String>,
    pub extra_dependencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub path: String,
    pub key: String,
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub kind: String,
    pub hooks: Vec<String>,
    pub valid: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct PluginManifest {
    key: Option<String>,
    name: Option<String>,
    version: Option<String>,
    description: Option<String>,
    kind: Option<String>,
    language: Option<String>,
    template_file: Option<String>,
    hooks: Option<PluginHooks>,
}

#[derive(Debug, Clone, Deserialize)]
struct PluginHooks {
    pre_build: Option<HookCommand>,
    post_build: Option<HookCommand>,
    lint_rule: Option<HookCommand>,
    asset_provider: Option<HookCommand>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum HookCommand {
    String(String),
    Args(Vec<String>),
}

#[derive(Debug, Clone, Copy)]
pub enum PluginHook {
    PreBuild,
    PostBuild,
    LintRule,
    AssetProvider,
}

struct LoadedPlugin {
    info: PluginInfo,
    manifest: PluginManifest,
    base_dir: PathBuf,
}

pub struct PluginContext<'a> {
    pub project_path: &'a Path,
    pub config: &'a MergedConfig,
    pub output: Option<&'a str>,
    pub target: Option<&'a str>,
}

pub fn supported_outputs() -> &'static [&'static str] {
    &["pdf", "html", "epub", "docx", "latex"]
}

pub fn default_all_outputs() -> Vec<String> {
    vec!["pdf", "html", "docx", "epub"]
        .into_iter()
        .map(str::to_string)
        .collect()
}

pub fn validate_config(project_path: &Path, config: &MergedConfig) -> Vec<ProjectIssue> {
    let mut issues = Vec::new();

    if let Some(from) = &config.from {
        let normalized = from.to_ascii_lowercase();
        if !matches!(normalized.as_str(), "markdown" | "md" | "latex" | "tex") {
            issues.push(error(
                format!("Unsupported project.from '{}'", from),
                None,
                None,
            ));
        }
    }

    if let Some(to) = &config.to {
        if !supported_outputs().contains(&to.to_ascii_lowercase().as_str()) {
            issues.push(error(
                format!("Unsupported project.to '{}'", to),
                None,
                None,
            ));
        }
    }

    for output in &config.outputs {
        if !supported_outputs().contains(&output.to_ascii_lowercase().as_str()) {
            issues.push(error(
                format!("Unsupported build.outputs item '{}'", output),
                None,
                None,
            ));
        }
    }

    for format in config.pandoc_format_options.keys() {
        if !is_supported_format_key(format) {
            issues.push(error(
                format!(
                    "Unsupported pandoc.format_options key '{}'. Supported keys: pdf, html, epub, docx, latex",
                    format
                ),
                Some(".omnidoc.toml".to_string()),
                None,
            ));
        }
    }

    if !config.latex_backend.is_empty()
        && !matches!(
            config.latex_backend.to_ascii_lowercase().as_str(),
            "latexmk" | "engine"
        )
    {
        issues.push(error(
            format!("Unsupported build.latex_backend '{}'", config.latex_backend),
            None,
            None,
        ));
    }

    if config.latex_backend.eq_ignore_ascii_case("engine") && config.max_latex_passes == 0 {
        issues.push(error(
            "build.max_latex_passes must be greater than 0 when build.latex_backend is engine"
                .to_string(),
            None,
            None,
        ));
    }

    if let Some(entry) = &config.entry {
        let entry_path = project_path.join(entry);
        if !entry_path.exists() {
            issues.push(error(
                format!("Configured entry file not found: {}", entry),
                Some(entry.clone()),
                None,
            ));
        }
    } else if !project_path.join("main.md").exists() && !project_path.join("main.tex").exists() {
        issues.push(error(
            "No entry configured and neither main.md nor main.tex exists".to_string(),
            None,
            None,
        ));
    }

    if let Some(lib_path) = &config.lib_path {
        if !Path::new(lib_path).exists() {
            issues.push(warning(
                format!(
                    "Configured omnidoc library path does not exist: {}",
                    lib_path
                ),
                Some(lib_path.clone()),
                None,
            ));
        }
    }

    if config.theme_name.is_some() {
        if let Err(theme_error) = load_selected_theme(config) {
            issues.push(error(
                format!("Invalid theme configuration: {}", theme_error),
                Some(".omnidoc.toml".to_string()),
                None,
            ));
        }
    } else if config.theme_version.is_some() || config.theme_compatibility.is_some() {
        issues.push(error(
            "theme.version and theme.compatibility require theme.name".to_string(),
            Some(".omnidoc.toml".to_string()),
            None,
        ));
    }
    if let Some(profile) = config.theme_compatibility.as_deref() {
        if !is_supported_epub_profile(profile) {
            issues.push(error(
                format!(
                    "Unsupported EPUB compatibility profile '{}'. Supported profiles: readium",
                    profile
                ),
                Some(".omnidoc.toml".to_string()),
                None,
            ));
        }
    }

    if let Some(metadata_file) = &config.metadata_file {
        check_configured_path(
            project_path,
            metadata_file,
            "Configured build.metadata_file not found",
            true,
            &mut issues,
        );
    }

    if let Some(css) = &config.pandoc_css {
        check_configured_css_path(
            project_path,
            css,
            config.lib_path.as_deref(),
            "Configured pandoc.css not found",
            &mut issues,
        );
    }

    if let Some(reference_doc) = &config.pandoc_reference_doc {
        check_configured_path(
            project_path,
            reference_doc,
            "Configured pandoc.reference_doc not found",
            true,
            &mut issues,
        );
    }

    if let Some(epub_css) = &config.pandoc_epub_css {
        check_configured_css_path(
            project_path,
            epub_css,
            config.lib_path.as_deref(),
            "Configured pandoc.epub_css not found",
            &mut issues,
        );
    }

    if let Some(data_dir) = &config.pandoc_data_dir {
        check_configured_path(
            project_path,
            data_dir,
            "Configured pandoc.data_dir not found",
            true,
            &mut issues,
        );
    }

    issues
}

pub fn lint_project(project_path: &Path) -> Vec<ProjectIssue> {
    let mut issues = Vec::new();
    let image_re = regex::Regex::new(r"!\[[^\]]*\]\(([^)]+)\)").expect("image regex");
    let link_re =
        regex::Regex::new(r"(?P<bang>!?)\[[^\]]+\]\((?P<target>[^)]+)\)").expect("link regex");
    let include_re = regex::Regex::new(r#"include(?:-code)?="([^"]+)""#).expect("include regex");

    for file in source_files(project_path) {
        let Ok(content) = fs::read_to_string(&file) else {
            continue;
        };
        let rel = display_relative(project_path, &file);
        for (line_index, line) in content.lines().enumerate() {
            let line_no = line_index + 1;
            for capture in image_re.captures_iter(line) {
                check_local_target(project_path, &file, &capture[1], &rel, line_no, &mut issues);
            }
            for capture in link_re.captures_iter(line) {
                if capture.name("bang").map(|m| m.as_str()) == Some("!") {
                    continue;
                }
                let target = capture.name("target").map(|m| m.as_str()).unwrap_or("");
                if is_local_path(target) {
                    check_local_target(project_path, &file, target, &rel, line_no, &mut issues);
                }
            }
            for capture in include_re.captures_iter(line) {
                check_local_target(project_path, &file, &capture[1], &rel, line_no, &mut issues);
            }
        }
    }

    issues
}

pub fn dependency_graph(project_path: &Path, config: &MergedConfig) -> DependencyGraph {
    let mut files = BTreeSet::new();
    let mut pending = Vec::new();
    let mut depfile_resources = BTreeMap::new();

    track_dependency(
        project_path,
        project_path,
        Path::new(".omnidoc.toml"),
        &mut files,
        &mut pending,
    );

    for configured in [
        config.entry.as_ref(),
        config.metadata_file.as_ref(),
        config.pandoc_css.as_ref(),
        config.pandoc_reference_doc.as_ref(),
        config.pandoc_epub_css.as_ref(),
        config.pandoc_template.as_ref(),
        config.pandoc_html_template.as_ref(),
        config.pandoc_latex_template.as_ref(),
        config.pandoc_epub_template.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        track_dependency(
            project_path,
            project_path,
            Path::new(configured),
            &mut files,
            &mut pending,
        );
    }

    let output_kind = PandocOutputKind::from_config(config).unwrap_or(PandocOutputKind::Pdf);
    let active_filters = output_kind.filters(config);
    for depfile in [
        active_filters
            .contains(&"include-files.lua")
            .then_some(INCLUDE_DEPFILE),
        active_filters
            .contains(&"include-code-files.lua")
            .then_some(INCLUDE_CODE_DEPFILE),
    ]
    .into_iter()
    .flatten()
    {
        load_depfile_dependencies(
            project_path,
            depfile,
            &mut files,
            &mut pending,
            &mut depfile_resources,
        );
    }

    while let Some(file) = pending.pop() {
        let Ok(content) = fs::read_to_string(&file) else {
            continue;
        };
        let base = file.parent().unwrap_or(project_path);
        for referenced in referenced_local_files(&content) {
            track_dependency(
                project_path,
                base,
                Path::new(&referenced),
                &mut files,
                &mut pending,
            );
            if matches!(output_kind, PandocOutputKind::Pdf | PandocOutputKind::Latex) {
                track_svg_pdf_sibling(
                    project_path,
                    base,
                    Path::new(&referenced),
                    &mut files,
                    &mut pending,
                );
            }
        }
    }

    let mut resources = resolved_build_resources(project_path, config);
    resources.extend(depfile_resources.into_values());
    resources.sort_by(|left, right| {
        (&left.logical_name, &left.path).cmp(&(&right.logical_name, &right.path))
    });
    resources
        .dedup_by(|left, right| left.logical_name == right.logical_name && left.path == right.path);
    DependencyGraph {
        files: files.into_iter().collect(),
        resources,
    }
}

fn track_svg_pdf_sibling(
    project_path: &Path,
    base: &Path,
    referenced: &Path,
    files: &mut BTreeSet<String>,
    pending: &mut Vec<PathBuf>,
) {
    if !referenced
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"))
    {
        return;
    }

    track_dependency(
        project_path,
        base,
        &referenced.with_extension("pdf"),
        files,
        pending,
    );
}

fn load_depfile_dependencies(
    project_path: &Path,
    depfile_name: &str,
    files: &mut BTreeSet<String>,
    pending: &mut Vec<PathBuf>,
    external: &mut BTreeMap<String, ResolvedResource>,
) {
    let depfile = project_path.join(CACHE_DIR).join(depfile_name);
    let Ok(metadata) = fs::metadata(&depfile) else {
        return;
    };
    if metadata.len() > 1024 * 1024 {
        return;
    }
    let Ok(content) = fs::read_to_string(&depfile) else {
        return;
    };
    let mut lines = content.lines();
    if lines.next() != Some("# omnidoc-depfile-v1") {
        return;
    }
    let canonical_project = project_path
        .canonicalize()
        .unwrap_or_else(|_| project_path.to_path_buf());
    for line in lines {
        let dependency = line.trim();
        if dependency.is_empty() || dependency.starts_with('#') {
            continue;
        }
        let path = PathBuf::from(dependency);
        let candidate = if path.is_absolute() {
            path
        } else {
            project_path.join(path)
        };
        let Ok(canonical) = candidate.canonicalize() else {
            continue;
        };
        if !canonical.is_file() {
            continue;
        }
        if canonical.starts_with(&canonical_project) {
            let relative = display_relative(&canonical_project, &canonical);
            if files.insert(relative) {
                pending.push(canonical);
            }
        } else {
            let path_text = canonical.to_string_lossy().to_string();
            external.insert(
                path_text.clone(),
                ResolvedResource {
                    logical_name: format!("include-depfile:{}", depfile_name),
                    resolved_from: "external".to_string(),
                    path: path_text,
                },
            );
        }
    }
}

fn resolved_build_resources(project_path: &Path, config: &MergedConfig) -> Vec<ResolvedResource> {
    let library_root = omnidoc_library_root(config);
    let mut resources = BTreeMap::<String, ResolvedResource>::new();
    let output_kind = PandocOutputKind::from_config(config).unwrap_or(PandocOutputKind::Pdf);
    let theme = load_selected_theme(config).ok().flatten();

    let manifest_path = library_root.join("manifest.toml");
    if let Some(path) = existing_path(manifest_path.clone()) {
        add_resolved_resource(
            &mut resources,
            project_path,
            &library_root,
            "omnidoc-libs-manifest".to_string(),
            path,
        );
    }
    if let Some(checksum_path) = library_contract(&library_root).1 {
        if let Some(path) = existing_path(checksum_path) {
            add_resolved_resource(
                &mut resources,
                project_path,
                &library_root,
                "omnidoc-libs-checksums".to_string(),
                path,
            );
        }
    }

    if let (Some(name), Some(_)) = (config.theme_name.as_deref(), theme.as_ref()) {
        if let Some(path) =
            existing_path(library_root.join("themes").join(format!("{}.toml", name)))
        {
            add_resolved_resource(
                &mut resources,
                project_path,
                &library_root,
                format!("theme-manifest:{}", name),
                path,
            );
        }
    }

    let mut resolved_filter_paths = BTreeSet::new();
    for filter in output_kind.filters(config) {
        // Keep this resolution identical to PandocBuilder::push_lua_filters:
        // filter names are relative to the shared filter directory.
        if let Some(path) =
            existing_path(library_root.join(pandoc::LIB_PANDOC_FILTERS).join(filter))
        {
            resolved_filter_paths.insert(path.clone());
            add_resolved_resource(
                &mut resources,
                project_path,
                &library_root,
                format!("lua-filter:{filter}"),
                path,
            );
        }
    }
    if let Some(theme) = &theme {
        for relative in &theme.resources.lua_filters {
            if let Some(path) = existing_path(library_root.join(relative)) {
                if !resolved_filter_paths.insert(path.clone()) {
                    continue;
                }
                add_resolved_resource(
                    &mut resources,
                    project_path,
                    &library_root,
                    format!("theme-lua-filter:{}", relative),
                    path,
                );
            }
        }
    }

    if output_kind.uses_latex_defaults() {
        if let Some(theme) = &theme {
            for relative in &theme.resources.latex_headers {
                if let Some(path) = existing_path(library_root.join(relative)) {
                    add_resolved_resource(
                        &mut resources,
                        project_path,
                        &library_root,
                        format!("theme-latex-header:{}", relative),
                        path,
                    );
                }
            }
            for relative in &theme.resources.latex_packages {
                if let Some(path) = existing_path(library_root.join(relative)) {
                    add_resolved_resource(
                        &mut resources,
                        project_path,
                        &library_root,
                        format!("theme-latex-package:{}", relative),
                        path,
                    );
                }
            }
        }
    }

    let data_dir = config
        .pandoc_data_dir
        .as_deref()
        .and_then(|path| resolve_resource_path(project_path, &library_root, path, None))
        .or_else(|| existing_path(library_root.join(pandoc::LIB_PANDOC_DATA)));
    if let Some(path) = data_dir {
        add_resolved_resource(
            &mut resources,
            project_path,
            &library_root,
            "pandoc-data-dir".to_string(),
            path,
        );
    }

    if matches!(output_kind, PandocOutputKind::Html | PandocOutputKind::Epub) {
        if let Some(path) = existing_path(library_root.join(pandoc::LIB_PANDOC_CSS_BASE)) {
            add_resolved_resource(
                &mut resources,
                project_path,
                &library_root,
                "omnidoc-base-css".to_string(),
                path,
            );
        }
    }

    let theme_css = theme.as_ref().and_then(|theme| match output_kind {
        PandocOutputKind::Html => theme.resources.html_css.first(),
        PandocOutputKind::Epub => theme.resources.epub_css.first(),
        _ => None,
    });
    let css = match output_kind {
        PandocOutputKind::Html => Some((
            "html-css",
            config
                .pandoc_css
                .as_deref()
                .or(theme_css.map(String::as_str)),
            pandoc::LIB_PANDOC_CSS_DEFAULT,
        )),
        PandocOutputKind::Epub => Some((
            "epub-css",
            config
                .pandoc_epub_css
                .as_deref()
                .or(config.pandoc_css.as_deref())
                .or(theme_css.map(String::as_str)),
            "pandoc/data/epub.css",
        )),
        _ => None,
    };
    if let Some((logical_name, configured, fallback)) = css {
        let path = configured
            .and_then(|value| {
                resolve_resource_path(project_path, &library_root, value, Some("pandoc/css"))
            })
            .or_else(|| existing_path(library_root.join(fallback)));
        if let Some(path) = path {
            add_resolved_resource(
                &mut resources,
                project_path,
                &library_root,
                logical_name.to_string(),
                path,
            );
        }
    }

    let template = match output_kind {
        PandocOutputKind::Pdf | PandocOutputKind::Latex => Some((
            "latex-template",
            config
                .pandoc_latex_template
                .as_deref()
                .or(config.pandoc_template.as_deref()),
            Some(pandoc::DEFAULT_TEMPLATE_LATEX),
        )),
        PandocOutputKind::Html => Some((
            "html-template",
            config
                .pandoc_html_template
                .as_deref()
                .or(config.pandoc_template.as_deref()),
            None,
        )),
        PandocOutputKind::Epub => Some((
            "epub-template",
            config
                .pandoc_epub_template
                .as_deref()
                .or(config.pandoc_template.as_deref()),
            None,
        )),
        PandocOutputKind::Docx => None,
    };
    if let Some((logical_name, configured, fallback)) = template {
        let selected = configured.or(fallback);
        if let Some(selected) = selected {
            if let Some(path) = resolve_resource_path(
                project_path,
                &library_root,
                selected,
                Some("pandoc/data/templates"),
            ) {
                add_resolved_resource(
                    &mut resources,
                    project_path,
                    &library_root,
                    logical_name.to_string(),
                    path,
                );
            }
        }
    }

    if output_kind == PandocOutputKind::Docx {
        if let Some(reference_doc) = config.pandoc_reference_doc.as_deref() {
            if let Some(path) =
                resolve_resource_path(project_path, &library_root, reference_doc, None)
            {
                add_resolved_resource(
                    &mut resources,
                    project_path,
                    &library_root,
                    "reference-doc".to_string(),
                    path,
                );
            }
        }
    }

    for (index, resource_path) in config.pandoc_resource_path.iter().enumerate() {
        if let Some(path) = resolve_resource_path(project_path, &library_root, resource_path, None)
        {
            add_resolved_resource(
                &mut resources,
                project_path,
                &library_root,
                format!("pandoc-resource-path:{index}"),
                path,
            );
        }
    }

    if config.pandoc_resource_path.is_empty() {
        if let Some(path) = existing_path(library_root.join(pandoc::LIB_PANDOC_CSL)) {
            add_resolved_resource(
                &mut resources,
                project_path,
                &library_root,
                "pandoc-csl-dir".to_string(),
                path,
            );
        }
    }

    if config
        .pandoc_lang
        .as_deref()
        .is_some_and(|lang| lang != "en")
    {
        let path = config
            .pandoc_crossref_yaml
            .as_deref()
            .and_then(|value| resolve_resource_path(project_path, &library_root, value, None))
            .or_else(|| existing_path(library_root.join(pandoc::LIB_PANDOC_CROSSREF_YAML)));
        if let Some(path) = path {
            add_resolved_resource(
                &mut resources,
                project_path,
                &library_root,
                "crossref-yaml".to_string(),
                path,
            );
        }
    }

    if output_kind.uses_latex_defaults() {
        if let Some(path) = existing_path(library_root.join("texmf")) {
            add_resolved_resource(
                &mut resources,
                project_path,
                &library_root,
                "texmf".to_string(),
                path,
            );
        }
    }

    resources.into_values().collect()
}

fn omnidoc_library_root(config: &MergedConfig) -> PathBuf {
    config
        .lib_path
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            dirs::data_local_dir()
                .map(|path| path.join("omnidoc"))
                .unwrap_or_else(|| PathBuf::from(".local/share/omnidoc"))
        })
}

fn resolve_resource_path(
    project_path: &Path,
    library_root: &Path,
    configured: &str,
    library_subdir: Option<&str>,
) -> Option<PathBuf> {
    let configured_path = PathBuf::from(configured);
    let candidates = if configured_path.is_absolute() {
        vec![configured_path]
    } else {
        let mut candidates = vec![project_path.join(&configured_path)];
        if let Some(subdir) = library_subdir {
            candidates.push(library_root.join(subdir).join(&configured_path));
        }
        candidates.push(library_root.join(&configured_path));
        candidates
    };
    candidates.into_iter().find_map(existing_path)
}

fn existing_path(path: PathBuf) -> Option<PathBuf> {
    path.exists().then_some(path)
}

fn add_resolved_resource(
    resources: &mut BTreeMap<String, ResolvedResource>,
    project_path: &Path,
    library_root: &Path,
    logical_name: String,
    path: PathBuf,
) {
    let path = path.canonicalize().unwrap_or(path);
    let canonical_project = project_path
        .canonicalize()
        .unwrap_or_else(|_| project_path.to_path_buf());
    let canonical_library = library_root
        .canonicalize()
        .unwrap_or_else(|_| library_root.to_path_buf());
    let resolved_from = if path.starts_with(&canonical_project) {
        "project"
    } else if path.starts_with(&canonical_library) {
        "omnidoc-libs"
    } else {
        "external"
    };
    let key = format!("{logical_name}:{}", path.display());
    resources.insert(
        key,
        ResolvedResource {
            logical_name,
            resolved_from: resolved_from.to_string(),
            path: path.to_string_lossy().to_string(),
        },
    );
}

fn track_dependency(
    project_path: &Path,
    base: &Path,
    referenced: &Path,
    files: &mut BTreeSet<String>,
    pending: &mut Vec<PathBuf>,
) {
    let referenced_text = referenced.to_string_lossy();
    let referenced = Path::new(referenced_text.split(['#', '?']).next().unwrap_or(""));
    if referenced.as_os_str().is_empty() {
        return;
    }

    let mut candidates = Vec::new();
    if referenced.is_absolute() {
        candidates.push(referenced.to_path_buf());
    } else {
        candidates.push(base.join(referenced));
        if base != project_path {
            candidates.push(project_path.join(referenced));
        }
    }

    for candidate in candidates {
        let resolved = if candidate.is_file() {
            candidate
        } else if candidate.extension().is_none() && candidate.with_extension("tex").is_file() {
            candidate.with_extension("tex")
        } else {
            continue;
        };
        let Ok(canonical_project) = project_path.canonicalize() else {
            return;
        };
        let Ok(canonical_file) = resolved.canonicalize() else {
            continue;
        };
        if !canonical_file.starts_with(&canonical_project) {
            continue;
        }
        let relative = display_relative(&canonical_project, &canonical_file);
        if files.insert(relative) {
            pending.push(canonical_file);
        }
        return;
    }
}

fn referenced_local_files(content: &str) -> Vec<String> {
    let patterns = [
        r#"!\[[^\]]*\]\(\s*<?([^)>\s]+)"#,
        r#"(?:include|include-code)=[\"']([^\"']+)[\"']"#,
        r#"(?:src|href)=[\"']([^\"']+)[\"']"#,
        r#"\\(?:input|include|includegraphics)(?:\[[^\]]*\])?\{([^}]+)\}"#,
        r#"url\(\s*[\"']?([^\)\"']+)[\"']?\s*\)"#,
        r#"@import\s+[\"']([^\"']+)[\"']"#,
        r#"(?m)^\s*(?:cover-image|bibliography|csl|include-before-body|include-after-body)\s*:\s*[\"']?([^\"'\s]+)"#,
    ];
    let mut references = BTreeSet::new();
    for pattern in patterns {
        let regex = regex::Regex::new(pattern).expect("dependency reference regex");
        for captures in regex.captures_iter(content) {
            let Some(target) = captures.get(1).map(|capture| capture.as_str().trim()) else {
                continue;
            };
            if is_local_path(target) && !target.starts_with("data:") {
                references.insert(target.trim_matches(['<', '>']).to_string());
            }
        }
    }

    for pattern in [
        r#"(?ms)^```[^\n]*\{[^}\n]*\.include[^}\n]*\}[^\n]*\n(.*?)^```\s*$"#,
        r#"(?ms)^~~~[^\n]*\{[^}\n]*\.include[^}\n]*\}[^\n]*\n(.*?)^~~~\s*$"#,
    ] {
        let regex = regex::Regex::new(pattern).expect("include block regex");
        for captures in regex.captures_iter(content) {
            let Some(body) = captures.get(1).map(|capture| capture.as_str()) else {
                continue;
            };
            for line in body.lines() {
                let target = line.trim();
                if !target.is_empty() && !target.starts_with("//") && is_local_path(target) {
                    references.insert(target.to_string());
                }
            }
        }
    }
    references.into_iter().collect()
}

pub fn input_digest(project_path: &Path, graph: &DependencyGraph) -> Result<String> {
    let mut hasher = Hasher::new();
    hash_dependency_files(project_path, graph, &mut hasher)?;
    hash_resolved_resources(graph, &mut hasher)?;
    Ok(format_digest(hasher.finalize()))
}

pub fn build_input_digest(
    project_path: &Path,
    graph: &DependencyGraph,
    config: &MergedConfig,
    output: &str,
) -> Result<String> {
    let mut hasher = Hasher::new();
    hash_dependency_files(project_path, graph, &mut hasher)?;
    hash_resolved_resources(graph, &mut hasher)?;
    for (label, value) in [
        ("output", format!("{output:?}")),
        ("from", format!("{:?}", config.from)),
        ("to", format!("{:?}", config.to)),
        ("target", format!("{:?}", config.target)),
        ("outdir", format!("{:?}", config.outdir)),
        ("author", format!("{:?}", config.author)),
        ("metadata_file", format!("{:?}", config.metadata_file)),
        ("latex_backend", format!("{:?}", config.latex_backend)),
        ("max_latex_passes", format!("{:?}", config.max_latex_passes)),
        ("figure_paths", format!("{:?}", config.figure_paths)),
        ("figure_output", format!("{:?}", config.figure_output)),
        ("theme_name", format!("{:?}", config.theme_name)),
        ("theme_version", format!("{:?}", config.theme_version)),
        (
            "theme_compatibility",
            format!("{:?}", config.theme_compatibility),
        ),
        ("pandoc_options", format!("{:?}", config.pandoc_options)),
        (
            "pandoc_format_options",
            format!("{:?}", config.pandoc_format_options),
        ),
        ("pandoc_css", format!("{:?}", config.pandoc_css)),
        (
            "pandoc_reference_doc",
            format!("{:?}", config.pandoc_reference_doc),
        ),
        ("pandoc_epub_css", format!("{:?}", config.pandoc_epub_css)),
        (
            "pandoc_from_format",
            format!("{:?}", config.pandoc_from_format),
        ),
        ("pandoc_to_format", format!("{:?}", config.pandoc_to_format)),
        (
            "pandoc_lua_filters",
            format!("{:?}", config.pandoc_lua_filters),
        ),
        ("pandoc_template", format!("{:?}", config.pandoc_template)),
        (
            "pandoc_html_template",
            format!("{:?}", config.pandoc_html_template),
        ),
        (
            "pandoc_latex_template",
            format!("{:?}", config.pandoc_latex_template),
        ),
        (
            "pandoc_epub_template",
            format!("{:?}", config.pandoc_epub_template),
        ),
        ("pandoc_data_dir", format!("{:?}", config.pandoc_data_dir)),
        (
            "pandoc_resource_path",
            format!("{:?}", config.pandoc_resource_path),
        ),
        (
            "pandoc_syntax_highlighting",
            format!("{:?}", config.pandoc_syntax_highlighting),
        ),
        (
            "pandoc_crossref_yaml",
            format!("{:?}", config.pandoc_crossref_yaml),
        ),
        (
            "pandoc_python_path",
            format!("{:?}", config.pandoc_python_path),
        ),
        (
            "pandoc_standalone",
            format!("{:?}", config.pandoc_standalone),
        ),
        (
            "pandoc_embed_resources",
            format!("{:?}", config.pandoc_embed_resources),
        ),
        ("pandoc_lang", format!("{:?}", config.pandoc_lang)),
        (
            "tool_paths",
            format!("{:?}", sorted_tool_paths(&config.tool_paths)),
        ),
    ] {
        hash_field(&mut hasher, label, value.as_bytes());
    }
    for (name, version) in toolchain_versions(config, output) {
        hash_field(&mut hasher, "toolchain-name", name.as_bytes());
        hash_field(&mut hasher, "toolchain-version", version.as_bytes());
    }
    Ok(format_digest(hasher.finalize()))
}

fn hash_dependency_files(
    project_path: &Path,
    graph: &DependencyGraph,
    hasher: &mut Hasher,
) -> Result<()> {
    for file in &graph.files {
        hash_field(hasher, "dependency", file.as_bytes());
        let path = project_path.join(file);
        if path.is_file() {
            hash_field(hasher, "content", &fs::read(&path)?);
        }
    }
    Ok(())
}

fn hash_resolved_resources(graph: &DependencyGraph, hasher: &mut Hasher) -> Result<()> {
    for resource in &graph.resources {
        hash_field(hasher, "resource-name", resource.logical_name.as_bytes());
        hash_field(hasher, "resource-origin", resource.resolved_from.as_bytes());
        hash_path(Path::new(&resource.path), hasher)?;
    }
    Ok(())
}

fn hash_path(path: &Path, hasher: &mut Hasher) -> Result<()> {
    if path.is_file() {
        hash_field(hasher, "file", &fs::read(path)?);
        return Ok(());
    }
    if path.is_dir() {
        let mut files = WalkDir::new(path)
            .into_iter()
            .flatten()
            .filter(|entry| entry.file_type().is_file())
            .map(|entry| entry.into_path())
            .collect::<Vec<_>>();
        files.sort();
        for file in files {
            let relative = display_relative(path, &file);
            hash_field(hasher, "relative-path", relative.as_bytes());
            hash_field(hasher, "file", &fs::read(file)?);
        }
    }
    Ok(())
}

pub(crate) fn content_digest(path: &Path) -> Result<String> {
    let mut hasher = Hasher::new();
    hash_path(path, &mut hasher)?;
    Ok(format_digest(hasher.finalize()))
}

fn hash_field(hasher: &mut Hasher, label: &str, value: &[u8]) {
    hasher.update(&(label.len() as u64).to_le_bytes());
    hasher.update(label.as_bytes());
    hasher.update(&(value.len() as u64).to_le_bytes());
    hasher.update(value);
}

fn format_digest(digest: blake3::Hash) -> String {
    format!("blake3:{digest}")
}

fn sorted_tool_paths(
    tool_paths: &std::collections::HashMap<String, Option<String>>,
) -> BTreeMap<String, Option<String>> {
    tool_paths
        .iter()
        .map(|(tool, path)| (tool.clone(), path.clone()))
        .collect()
}

pub fn cache_hit(project_path: &Path, output: &str, digest: &str) -> bool {
    let path = cache_path(project_path, output);
    let Ok(content) = fs::read_to_string(path) else {
        return false;
    };
    serde_json::from_str::<BuildCache>(&content)
        .map(|cache| cache.cache_version == CACHE_VERSION && cache.input_digest == digest)
        .unwrap_or(false)
}

pub fn write_cache(project_path: &Path, output: &str, digest: &str) -> Result<()> {
    fs::create_dir_all(project_path.join(CACHE_DIR))?;
    let cache = BuildCache {
        cache_version: CACHE_VERSION,
        input_digest: digest.to_string(),
    };
    let content =
        serde_json::to_string_pretty(&cache).map_err(|err| OmniDocError::Other(err.to_string()))?;
    fs::write(cache_path(project_path, output), content)?;
    Ok(())
}

pub fn write_report(
    project_path: &Path,
    config: &MergedConfig,
    report: &BuildReport,
) -> Result<()> {
    write_reports(project_path, config, std::slice::from_ref(report))
}

pub fn write_reports(
    project_path: &Path,
    config: &MergedConfig,
    reports: &[BuildReport],
) -> Result<()> {
    let outdir = config
        .outdir
        .as_ref()
        .map(|outdir| project_path.join(outdir))
        .unwrap_or_else(|| project_path.join("build"));
    fs::create_dir_all(&outdir)?;
    let document = BuildReportDocument {
        omnidoc_version: env!("CARGO_PKG_VERSION").to_string(),
        generated_at_unix: current_timestamp_unix(),
        reports: reports.to_vec(),
    };
    let content = serde_json::to_string_pretty(&document)
        .map_err(|err| OmniDocError::Other(err.to_string()))?;
    fs::write(outdir.join(REPORT_FILE), content)?;
    Ok(())
}

pub fn write_lock(
    project_path: &Path,
    config: &MergedConfig,
    graph: &DependencyGraph,
) -> Result<()> {
    let output = config
        .to
        .as_deref()
        .or(config.pandoc_to_format.as_deref())
        .unwrap_or("pdf");
    write_lock_targets(
        project_path,
        &[LockTargetInput {
            output,
            config,
            graph,
        }],
    )
}

pub fn write_lock_targets(project_path: &Path, inputs: &[LockTargetInput<'_>]) -> Result<()> {
    let Some(first) = inputs.first() else {
        return Err(OmniDocError::Other(
            "cannot write a lock file without targets".to_string(),
        ));
    };
    let mut targets = BTreeMap::new();
    let mut all_resources = BTreeMap::new();
    for input in inputs {
        let target = locked_target(project_path, input)?;
        for resource in &target.resources {
            all_resources.insert(
                format!(
                    "{}:{}:{}",
                    resource.logical_name, resource.resolved_from, resource.digest
                ),
                resource.clone(),
            );
        }
        targets.insert(input.output.to_ascii_lowercase(), target);
    }
    let resources = all_resources.into_values().collect::<Vec<_>>();
    let lock = LockFile {
        lock_version: LOCK_VERSION,
        omnidoc_version: env!("CARGO_PKG_VERSION").to_string(),
        library: locked_library(first.config, &resources),
        toolchain: combined_toolchain_versions(inputs),
        targets,
    };
    let content =
        toml::to_string_pretty(&lock).map_err(|err| OmniDocError::Other(err.to_string()))?;
    fs::write(project_path.join(LOCK_FILE), content)?;
    Ok(())
}

pub fn check_lock(
    project_path: &Path,
    config: &MergedConfig,
    graph: &DependencyGraph,
) -> Result<LockStatus> {
    let output = config
        .to
        .as_deref()
        .or(config.pandoc_to_format.as_deref())
        .unwrap_or("pdf");
    check_lock_targets(
        project_path,
        &[LockTargetInput {
            output,
            config,
            graph,
        }],
    )
}

pub fn check_lock_targets(
    project_path: &Path,
    inputs: &[LockTargetInput<'_>],
) -> Result<LockStatus> {
    let lock_path = project_path.join(LOCK_FILE);
    if !lock_path.exists() {
        let mut targets = BTreeMap::new();
        for input in inputs {
            let expected = locked_target(project_path, input)?;
            targets.insert(
                input.output.to_ascii_lowercase(),
                LockTargetStatus {
                    up_to_date: false,
                    expected_digest: expected.input_digest,
                    actual_digest: None,
                    missing_dependencies: expected.dependencies,
                    extra_dependencies: Vec::new(),
                },
            );
        }
        return Ok(LockStatus {
            exists: false,
            up_to_date: false,
            library_up_to_date: false,
            toolchain_up_to_date: false,
            missing_targets: inputs
                .iter()
                .map(|input| input.output.to_ascii_lowercase())
                .collect(),
            extra_targets: Vec::new(),
            targets,
        });
    }

    let content = fs::read_to_string(&lock_path)?;
    let lock: LockFile =
        toml::from_str(&content).map_err(|err| OmniDocError::Other(err.to_string()))?;
    let expected_names = inputs
        .iter()
        .map(|input| input.output.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    let actual_names = lock.targets.keys().cloned().collect::<BTreeSet<_>>();
    let missing_targets = expected_names
        .difference(&actual_names)
        .cloned()
        .collect::<Vec<_>>();
    let extra_targets = actual_names
        .difference(&expected_names)
        .cloned()
        .collect::<Vec<_>>();
    let mut statuses = BTreeMap::new();
    let mut all_resources = BTreeMap::new();
    for input in inputs {
        let name = input.output.to_ascii_lowercase();
        let expected = locked_target(project_path, input)?;
        for resource in &expected.resources {
            all_resources.insert(
                format!(
                    "{}:{}:{}",
                    resource.logical_name, resource.resolved_from, resource.digest
                ),
                resource.clone(),
            );
        }
        let actual = lock.targets.get(&name);
        let expected_dependencies = expected
            .dependencies
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let actual_dependencies = actual
            .map(|target| target.dependencies.iter().cloned().collect::<BTreeSet<_>>())
            .unwrap_or_default();
        let missing_dependencies = expected_dependencies
            .difference(&actual_dependencies)
            .cloned()
            .collect::<Vec<_>>();
        let extra_dependencies = actual_dependencies
            .difference(&expected_dependencies)
            .cloned()
            .collect::<Vec<_>>();
        let target_up_to_date = actual.is_some_and(|target| {
            target.input_digest == expected.input_digest
                && target.resources == expected.resources
                && missing_dependencies.is_empty()
                && extra_dependencies.is_empty()
        });
        statuses.insert(
            name,
            LockTargetStatus {
                up_to_date: target_up_to_date,
                expected_digest: expected.input_digest,
                actual_digest: actual.map(|target| target.input_digest.clone()),
                missing_dependencies,
                extra_dependencies,
            },
        );
    }
    let resources = all_resources.into_values().collect::<Vec<_>>();
    let first_config = inputs.first().map(|input| input.config);
    let library_up_to_date =
        first_config.is_some_and(|config| lock.library == locked_library(config, &resources));
    let toolchain_up_to_date = lock.toolchain == combined_toolchain_versions(inputs);
    let up_to_date = lock.lock_version == LOCK_VERSION
        && missing_targets.is_empty()
        && extra_targets.is_empty()
        && library_up_to_date
        && toolchain_up_to_date
        && statuses.values().all(|status| status.up_to_date);

    Ok(LockStatus {
        exists: true,
        up_to_date,
        library_up_to_date,
        toolchain_up_to_date,
        missing_targets,
        extra_targets,
        targets: statuses,
    })
}

fn locked_target(project_path: &Path, input: &LockTargetInput<'_>) -> Result<LockedTarget> {
    Ok(LockedTarget {
        input_digest: build_input_digest(project_path, input.graph, input.config, input.output)?,
        resources: locked_resources(input.graph)?,
        dependencies: input.graph.files.clone(),
    })
}

fn locked_resources(graph: &DependencyGraph) -> Result<Vec<LockedResource>> {
    graph
        .resources
        .iter()
        .map(|resource| {
            Ok(LockedResource {
                logical_name: resource.logical_name.clone(),
                resolved_from: resource.resolved_from.clone(),
                digest: content_digest(Path::new(&resource.path))?,
            })
        })
        .collect()
}

fn locked_library(config: &MergedConfig, resources: &[LockedResource]) -> Option<LockedLibrary> {
    let library_resources = resources
        .iter()
        .filter(|resource| resource.resolved_from == "omnidoc-libs")
        .collect::<Vec<_>>();
    if library_resources.is_empty() {
        return None;
    }
    let mut hasher = Hasher::new();
    for resource in library_resources {
        hash_field(
            &mut hasher,
            "logical-name",
            resource.logical_name.as_bytes(),
        );
        hash_field(&mut hasher, "digest", resource.digest.as_bytes());
    }
    let library_root = omnidoc_library_root(config);
    let (version, checksum_path) = library_contract(&library_root);
    Some(LockedLibrary {
        version,
        revision: git_revision(&library_root),
        manifest_digest: existing_path(library_root.join("manifest.toml"))
            .and_then(|path| content_digest(&path).ok()),
        checksums_digest: checksum_path
            .and_then(existing_path)
            .and_then(|path| content_digest(&path).ok()),
        digest: format_digest(hasher.finalize()),
    })
}

fn library_contract(library_root: &Path) -> (Option<String>, Option<PathBuf>) {
    let manifest_path = library_root.join("manifest.toml");
    let Ok(content) = fs::read_to_string(manifest_path) else {
        return (None, None);
    };
    let Ok(manifest) = toml::from_str::<toml::Value>(&content) else {
        return (None, None);
    };
    let version = manifest
        .get("version")
        .and_then(toml::Value::as_str)
        .map(str::to_string);
    let checksum_path = manifest
        .get("checksum_file")
        .and_then(toml::Value::as_str)
        .map(|relative| library_root.join(relative));
    (version, checksum_path)
}

fn git_revision(path: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["-C", &path.to_string_lossy(), "rev-parse", "HEAD"])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn toolchain_versions(config: &MergedConfig, output: &str) -> BTreeMap<String, String> {
    let latex_engine = config
        .tool_paths
        .get("latex_engine")
        .and_then(|value| value.clone())
        .unwrap_or_else(|| "xelatex".to_string());
    let mut versions = [
        ("pandoc", configured_tool(config, "pandoc", "pandoc")),
        (
            "pandoc_crossref",
            configured_tool(config, "pandoc_crossref", "pandoc-crossref"),
        ),
    ]
    .into_iter()
    .map(|(name, program)| (name.to_string(), command_version(&program)))
    .collect::<BTreeMap<_, _>>();
    let output_kind = PandocOutputKind::from_requested(Some(output)).ok();
    if output_kind == Some(PandocOutputKind::Pdf) {
        versions.insert("latex_engine".to_string(), command_version(&latex_engine));
        if let Ok(Some(theme)) = load_selected_theme(config) {
            for font in theme.requirements.fonts {
                versions.insert(format!("font:{font}"), font_identity(&font));
            }
        }
    }
    versions
}

fn combined_toolchain_versions(inputs: &[LockTargetInput<'_>]) -> BTreeMap<String, String> {
    let mut versions = BTreeMap::new();
    for input in inputs {
        versions.extend(toolchain_versions(input.config, input.output));
    }
    versions
}

fn font_identity(requested: &str) -> String {
    let output = match Command::new("fc-match")
        .args([
            "--format",
            "%{family}|%{style}|%{fontversion}|%{file}\\n",
            "--",
            requested,
        ])
        .output()
    {
        Ok(output) if output.status.success() => output,
        _ => return "unavailable".to_string(),
    };
    let line = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or("")
        .to_string();
    let mut fields = line.splitn(4, '|');
    let family = fields.next().unwrap_or("").trim();
    let style = fields.next().unwrap_or("").trim();
    let version = fields.next().unwrap_or("").trim();
    let path = Path::new(fields.next().unwrap_or("").trim());
    if !font_family_matches(requested, family) {
        return format!("missing;fallback={family}");
    }
    let file = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown");
    let digest = content_digest(path).unwrap_or_else(|_| "unavailable".to_string());
    format!("family={family};style={style};fontversion={version};file={file};digest={digest}")
}

fn configured_tool(config: &MergedConfig, key: &str, fallback: &str) -> String {
    config
        .tool_paths
        .get(key)
        .and_then(|value| value.clone())
        .unwrap_or_else(|| fallback.to_string())
}

fn command_version(program: &str) -> String {
    Command::new(program)
        .arg("--version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .next()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "unavailable".to_string())
}

pub fn discovered_plugins(project_path: &Path, config: &MergedConfig) -> Vec<PluginInfo> {
    loaded_plugins(project_path, config)
        .into_iter()
        .map(|plugin| plugin.info)
        .collect()
}

pub fn run_plugin_hook(context: &PluginContext<'_>, hook: PluginHook) -> Result<()> {
    for plugin in loaded_plugins(context.project_path, context.config)
        .into_iter()
        .filter(|plugin| plugin.info.valid)
    {
        let Some(command) = plugin_hook_command(&plugin.manifest, hook) else {
            continue;
        };
        run_hook_command(context, &plugin, command, hook)?;
    }
    Ok(())
}

pub fn run_plugin_lint_rules(project_path: &Path, config: &MergedConfig) -> Vec<ProjectIssue> {
    let context = PluginContext {
        project_path,
        config,
        output: None,
        target: None,
    };
    let mut issues = Vec::new();
    for plugin in loaded_plugins(project_path, config)
        .into_iter()
        .filter(|plugin| plugin.info.valid)
    {
        let Some(command) = plugin_hook_command(&plugin.manifest, PluginHook::LintRule) else {
            continue;
        };
        match run_hook_command_capture(&context, &plugin, command, PluginHook::LintRule) {
            Ok(output) => issues.extend(parse_lint_rule_output(&plugin, &output)),
            Err(err) => issues.push(error(
                format!("Plugin lint_rule failed for {}: {}", plugin.info.key, err),
                Some(plugin.info.path.clone()),
                None,
            )),
        }
    }
    issues
}

fn loaded_plugins(project_path: &Path, config: &MergedConfig) -> Vec<LoadedPlugin> {
    let mut plugins = Vec::new();
    for base in [
        config.template_dir.as_ref().map(PathBuf::from),
        config
            .lib_path
            .as_ref()
            .map(|path| Path::new(path).join("templates")),
        Some(project_path.join("plugins")),
    ]
    .into_iter()
    .flatten()
    {
        if !base.exists() {
            continue;
        }
        for manifest_path in plugin_manifest_paths(&base) {
            plugins.push(load_plugin_manifest(&manifest_path));
        }
    }
    plugins.sort_by(|left, right| left.info.path.cmp(&right.info.path));
    plugins.dedup_by(|left, right| left.info.path == right.info.path);
    plugins
}

pub fn has_errors(issues: &[ProjectIssue]) -> bool {
    issues
        .iter()
        .any(|issue| issue.severity == IssueSeverity::Error)
}

pub fn has_warnings_or_errors(issues: &[ProjectIssue]) -> bool {
    issues
        .iter()
        .any(|issue| issue.severity != IssueSeverity::Info)
}

pub fn print_issues(issues: &[ProjectIssue]) {
    for issue in issues {
        let severity = match issue.severity {
            IssueSeverity::Error => "error",
            IssueSeverity::Warning => "warning",
            IssueSeverity::Info => "info",
        };
        if let Some(path) = &issue.path {
            if let Some(line) = issue.line {
                println!("{}:{}: {}: {}", path, line, severity, issue.message);
            } else {
                println!("{}: {}: {}", path, severity, issue.message);
            }
        } else {
            println!("{}: {}", severity, issue.message);
        }
    }
}

pub fn build_report(context: BuildReportContext<'_>) -> BuildReport {
    let toolchain = toolchain_versions(context.config, &context.output);
    BuildReport {
        output: context.output,
        target: context.target,
        skipped: context.skipped,
        cache_reason: context.cache_reason,
        duration_ms: context.duration_ms,
        input_digest: context.input_digest,
        artifact_digest: context
            .artifact
            .is_file()
            .then(|| content_digest(context.artifact).ok())
            .flatten(),
        compatibility: context.compatibility,
        dependencies: context.graph.files.clone(),
        resources: locked_resources(context.graph).unwrap_or_default(),
        toolchain,
        issues: context.issues,
        timestamp_unix: current_timestamp_unix(),
    }
}

fn current_timestamp_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn source_files(project_path: &Path) -> Vec<PathBuf> {
    WalkDir::new(project_path)
        .into_iter()
        .filter_entry(|entry| should_descend(entry.path(), project_path))
        .flatten()
        .filter(|entry| entry.file_type().is_file() && is_source_file(entry.path()))
        .map(|entry| entry.into_path())
        .collect()
}

fn should_descend(path: &Path, project_path: &Path) -> bool {
    if path == project_path {
        return true;
    }
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    !matches!(
        name,
        ".git" | "build" | "target" | ".target" | ".cache" | CACHE_DIR | "node_modules"
    )
}

fn is_source_file(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|value| value.to_str()) else {
        return false;
    };
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "md" | "markdown"
            | "tex"
            | "bib"
            | "cls"
            | "sty"
            | "yaml"
            | "yml"
            | "json"
            | "drawio"
            | "dot"
            | "mmd"
            | "puml"
            | "plantuml"
            | "svg"
            | "png"
            | "jpg"
            | "jpeg"
            | "pdf"
            | "csv"
            | "tsv"
    )
}

fn check_local_target(
    project_path: &Path,
    source_file: &Path,
    target: &str,
    rel: &str,
    line: usize,
    issues: &mut Vec<ProjectIssue>,
) {
    if !is_local_path(target) {
        return;
    }
    let target = target.split('#').next().unwrap_or(target);
    if target.is_empty() {
        return;
    }
    let base = source_file.parent().unwrap_or(project_path);
    if !base.join(target).exists() && !project_path.join(target).exists() {
        issues.push(warning(
            format!("Referenced local resource not found: {}", target),
            Some(rel.to_string()),
            Some(line),
        ));
    }
}

fn check_configured_path(
    project_path: &Path,
    configured_path: &str,
    message: &str,
    is_error: bool,
    issues: &mut Vec<ProjectIssue>,
) {
    let path = Path::new(configured_path);
    let exists = if path.is_absolute() {
        path.exists()
    } else {
        project_path.join(path).exists() || path.exists()
    };

    if exists {
        return;
    }

    let issue_message = format!("{}: {}", message, configured_path);
    let issue = if is_error {
        error(issue_message, Some(configured_path.to_string()), None)
    } else {
        warning(issue_message, Some(configured_path.to_string()), None)
    };
    issues.push(issue);
}

fn check_configured_css_path(
    project_path: &Path,
    configured_path: &str,
    lib_path: Option<&str>,
    message: &str,
    issues: &mut Vec<ProjectIssue>,
) {
    let path = Path::new(configured_path);
    let project_exists = if path.is_absolute() {
        path.exists()
    } else {
        project_path.join(path).exists() || path.exists()
    };
    let library_root = lib_path
        .map(PathBuf::from)
        .or_else(|| dirs::data_local_dir().map(|path| path.join("omnidoc")));
    let shared_exists = library_root
        .map(|root| root.join("pandoc/css").join(path).exists())
        .unwrap_or(false);
    if project_exists || shared_exists {
        return;
    }
    issues.push(warning(
        format!("{}: {}", message, configured_path),
        Some(configured_path.to_string()),
        None,
    ));
}

fn plugin_manifest_paths(base: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for entry in WalkDir::new(base)
        .max_depth(3)
        .into_iter()
        .flatten()
        .filter(|entry| entry.file_type().is_file())
    {
        let path = entry.path();
        let file_name = path.file_name().and_then(|name| name.to_str());
        let parent_name = path
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|name| name.to_str());
        let is_manifest = file_name == Some("manifest.toml")
            || (parent_name == Some("manifests")
                && path.extension().and_then(|ext| ext.to_str()) == Some("toml"));
        if is_manifest {
            paths.push(path.to_path_buf());
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

fn load_plugin_manifest(path: &Path) -> LoadedPlugin {
    let base_dir = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let fallback_key = path
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
        .or_else(|| path.file_stem().and_then(|name| name.to_str()))
        .unwrap_or("plugin")
        .to_string();
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) => {
            return invalid_loaded_plugin(
                path,
                base_dir,
                fallback_key,
                format!("Failed to read manifest: {}", err),
            );
        }
    };
    let manifest = match toml::from_str::<PluginManifest>(&content) {
        Ok(manifest) => manifest,
        Err(err) => {
            return invalid_loaded_plugin(
                path,
                base_dir,
                fallback_key,
                format!("Failed to parse manifest: {}", err),
            );
        }
    };

    let key = manifest.key.clone().unwrap_or(fallback_key);
    let kind = manifest.kind.clone().unwrap_or_else(|| {
        if manifest.template_file.is_some() {
            "template".to_string()
        } else {
            "plugin".to_string()
        }
    });
    let error = validate_plugin_manifest(path, &manifest);
    let hooks = manifest_hook_names(&manifest);
    let info = PluginInfo {
        path: path.display().to_string(),
        key,
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        description: manifest.description.clone(),
        kind,
        hooks,
        valid: error.is_none(),
        error,
    };

    LoadedPlugin {
        info,
        manifest,
        base_dir,
    }
}

fn invalid_loaded_plugin(
    path: &Path,
    base_dir: PathBuf,
    key: String,
    error: String,
) -> LoadedPlugin {
    LoadedPlugin {
        info: PluginInfo {
            path: path.display().to_string(),
            key,
            name: None,
            version: None,
            description: None,
            kind: "plugin".to_string(),
            hooks: Vec::new(),
            valid: false,
            error: Some(error),
        },
        manifest: PluginManifest {
            key: None,
            name: None,
            version: None,
            description: None,
            kind: None,
            language: None,
            template_file: None,
            hooks: None,
        },
        base_dir,
    }
}

fn plugin_hook_command(manifest: &PluginManifest, hook: PluginHook) -> Option<&HookCommand> {
    let hooks = manifest.hooks.as_ref()?;
    match hook {
        PluginHook::PreBuild => hooks.pre_build.as_ref(),
        PluginHook::PostBuild => hooks.post_build.as_ref(),
        PluginHook::LintRule => hooks.lint_rule.as_ref(),
        PluginHook::AssetProvider => hooks.asset_provider.as_ref(),
    }
}

fn manifest_hook_names(manifest: &PluginManifest) -> Vec<String> {
    let Some(hooks) = &manifest.hooks else {
        return Vec::new();
    };
    let mut names = Vec::new();
    if hooks.asset_provider.is_some() {
        names.push("asset_provider".to_string());
    }
    if hooks.pre_build.is_some() {
        names.push("pre_build".to_string());
    }
    if hooks.post_build.is_some() {
        names.push("post_build".to_string());
    }
    if hooks.lint_rule.is_some() {
        names.push("lint_rule".to_string());
    }
    names
}

fn run_hook_command(
    context: &PluginContext<'_>,
    plugin: &LoadedPlugin,
    command: &HookCommand,
    hook: PluginHook,
) -> Result<()> {
    run_hook_command_capture(context, plugin, command, hook).map(|_| ())
}

fn run_hook_command_capture(
    context: &PluginContext<'_>,
    plugin: &LoadedPlugin,
    command: &HookCommand,
    hook: PluginHook,
) -> Result<String> {
    let argv = hook_argv(command);
    if argv.is_empty() {
        return Err(OmniDocError::Project(format!(
            "Plugin hook command is empty: {}",
            plugin.info.key
        )));
    }

    let program = resolve_hook_program(&plugin.base_dir, &argv[0]);
    let output = Command::new(&program)
        .args(&argv[1..])
        .current_dir(context.project_path)
        .env("OMNIDOC_PROJECT_DIR", context.project_path)
        .env("OMNIDOC_PLUGIN_DIR", &plugin.base_dir)
        .env("OMNIDOC_PLUGIN_KEY", &plugin.info.key)
        .env("OMNIDOC_HOOK", hook_name(hook))
        .env("OMNIDOC_OUTPUT", context.output.unwrap_or(""))
        .env("OMNIDOC_TARGET", context.target.unwrap_or(""))
        .output()
        .map_err(|err| {
            OmniDocError::Project(format!(
                "Failed to execute plugin hook {} for {}: {}",
                hook_name(hook),
                plugin.info.key,
                err
            ))
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    if output.status.success() {
        return Ok(stdout);
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(OmniDocError::Project(format!(
        "Plugin hook {} failed for {} with status {}\nstdout:\n{}\nstderr:\n{}",
        hook_name(hook),
        plugin.info.key,
        output.status,
        compact_snippet(&stdout),
        compact_snippet(&stderr)
    )))
}

fn hook_argv(command: &HookCommand) -> Vec<String> {
    match command {
        HookCommand::String(command) => command.split_whitespace().map(str::to_string).collect(),
        HookCommand::Args(args) => args.clone(),
    }
}

fn resolve_hook_program(base_dir: &Path, program: &str) -> PathBuf {
    let path = Path::new(program);
    if path.is_absolute() {
        return path.to_path_buf();
    }
    let local = base_dir.join(path);
    if local.exists() {
        local
    } else {
        path.to_path_buf()
    }
}

fn hook_name(hook: PluginHook) -> &'static str {
    match hook {
        PluginHook::PreBuild => "pre_build",
        PluginHook::PostBuild => "post_build",
        PluginHook::LintRule => "lint_rule",
        PluginHook::AssetProvider => "asset_provider",
    }
}

fn parse_lint_rule_output(plugin: &LoadedPlugin, output: &str) -> Vec<ProjectIssue> {
    output
        .lines()
        .filter_map(|line| parse_lint_rule_line(plugin, line))
        .collect()
}

fn parse_lint_rule_line(plugin: &LoadedPlugin, line: &str) -> Option<ProjectIssue> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    let mut parts = line.splitn(5, ':');
    let severity = match parts
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "error" => IssueSeverity::Error,
        "warning" | "warn" => IssueSeverity::Warning,
        "info" => IssueSeverity::Info,
        _ => {
            return Some(warning(
                format!("Plugin {}: {}", plugin.info.key, line),
                Some(plugin.info.path.clone()),
                None,
            ));
        }
    };
    let path = parts
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let line_no = parts
        .next()
        .and_then(|value| value.trim().parse::<usize>().ok());
    let column = parts.next().map(str::trim).unwrap_or("1");
    let message = parts.next().map(str::trim).unwrap_or("");
    let message = if message.is_empty() {
        format!("Plugin {} reported an issue", plugin.info.key)
    } else {
        format!(
            "Plugin {}: {} (column {})",
            plugin.info.key, message, column
        )
    };

    Some(ProjectIssue {
        severity,
        message,
        path: path.map(str::to_string),
        line: line_no,
    })
}

fn compact_snippet(input: &str) -> String {
    let snippet = input.split_whitespace().collect::<Vec<_>>().join(" ");
    if snippet.chars().count() > 500 {
        snippet.chars().take(497).collect::<String>() + "..."
    } else {
        snippet
    }
}

fn validate_plugin_manifest(path: &Path, manifest: &PluginManifest) -> Option<String> {
    if let Some(language) = &manifest.language {
        if !matches!(language.to_ascii_lowercase().as_str(), "markdown" | "latex") {
            return Some(format!("Unsupported template language: {}", language));
        }
    }

    if let Some(template_file) = &manifest.template_file {
        let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
        if !base_dir.join(template_file).exists() {
            return Some(format!("Template file not found: {}", template_file));
        }
        if manifest.language.is_none() {
            return Some("Template plugins must declare language".to_string());
        }
    }

    if let Some(hooks) = &manifest.hooks {
        for (name, command) in [
            ("asset_provider", hooks.asset_provider.as_ref()),
            ("pre_build", hooks.pre_build.as_ref()),
            ("post_build", hooks.post_build.as_ref()),
            ("lint_rule", hooks.lint_rule.as_ref()),
        ] {
            let Some(command) = command else {
                continue;
            };
            if let Some(error) = validate_hook_command(path, command) {
                return Some(format!("Invalid {} hook: {}", name, error));
            }
        }
    }

    None
}

fn validate_hook_command(manifest_path: &Path, command: &HookCommand) -> Option<String> {
    let argv = hook_argv(command);
    if argv.is_empty() {
        return Some("command is empty".to_string());
    }
    let program = Path::new(&argv[0]);
    if program.components().count() <= 1 && !program.is_absolute() {
        return None;
    }
    let base_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let resolved = resolve_hook_program(base_dir, &argv[0]);
    if resolved.exists() {
        None
    } else {
        Some(format!("command not found: {}", argv[0]))
    }
}

fn is_local_path(target: &str) -> bool {
    !target.starts_with("http://")
        && !target.starts_with("https://")
        && !target.starts_with("mailto:")
        && !target.starts_with('#')
}

fn cache_path(project_path: &Path, output: &str) -> PathBuf {
    project_path
        .join(CACHE_DIR)
        .join(format!("build-{}.json", output))
}

fn display_relative(project_path: &Path, path: &Path) -> String {
    path.strip_prefix(project_path)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

fn error(message: String, path: Option<String>, line: Option<usize>) -> ProjectIssue {
    ProjectIssue {
        severity: IssueSeverity::Error,
        message,
        path,
        line,
    }
}

fn warning(message: String, path: Option<String>, line: Option<usize>) -> ProjectIssue {
    ProjectIssue {
        severity: IssueSeverity::Warning,
        message,
        path,
        line,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_input_digest, build_report, cache_hit, dependency_graph, hook_argv,
        manifest_hook_names, parse_lint_rule_output, supported_outputs, validate_config,
        validate_hook_command, write_cache, write_lock, write_lock_targets, HookCommand,
        LoadedPlugin, LockFile, LockTargetInput, PluginHooks, PluginInfo, PluginManifest,
        CACHE_DIR, INCLUDE_DEPFILE,
    };
    use crate::config::MergedConfig;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_project(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("omnidoc-{name}-{}-{nonce}", std::process::id()))
    }

    #[test]
    fn validates_unsupported_output() {
        let config = MergedConfig {
            to: Some("unknown".to_string()),
            ..Default::default()
        };
        let issues = validate_config(Path::new("."), &config);
        assert!(issues
            .iter()
            .any(|issue| issue.message.contains("Unsupported")));
    }

    #[test]
    fn lists_core_outputs() {
        assert!(supported_outputs().contains(&"pdf"));
        assert!(supported_outputs().contains(&"html"));
    }

    #[test]
    fn dependency_graph_tracks_reachable_inputs_not_every_project_file() {
        let project = temporary_project("dependency-graph");
        fs::create_dir_all(project.join("assets")).expect("assets dir");
        fs::create_dir_all(project.join("chapters/nested")).expect("chapters dir");
        fs::create_dir_all(project.join("styles")).expect("styles dir");
        fs::create_dir_all(project.join("raw")).expect("raw dir");
        fs::create_dir_all(project.join("output/pdf")).expect("output dir");
        fs::create_dir_all(project.join("tmp")).expect("tmp dir");
        fs::write(
            project.join(".omnidoc.toml"),
            "[project]\nentry='main.md'\n",
        )
        .expect("config");
        fs::write(
            project.join("main.md"),
            "# Book\n\n```{.include format=\"markdown\"}\nchapters/chapter.md\n```\n",
        )
        .expect("entry");
        fs::write(
            project.join("chapters/chapter.md"),
            "# Chapter\n\n![used](../assets/used.png)\n\n```{.include}\nnested/part.md\n```\n",
        )
        .expect("chapter");
        fs::write(project.join("chapters/nested/part.md"), "## Nested\n").expect("nested chapter");
        fs::write(
            project.join("styles/book-metadata.yaml"),
            "cover-image: assets/cover.png\nheader-includes:\n  - \\input{styles/theme.tex}\n",
        )
        .expect("metadata");
        fs::write(project.join("styles/book.css"), "body { color: black; }\n").expect("css");
        fs::write(project.join("styles/theme.tex"), "\\usepackage{xcolor}\n").expect("theme");
        fs::write(project.join("assets/used.png"), b"used").expect("used image");
        fs::write(project.join("assets/cover.png"), b"cover").expect("cover image");
        fs::write(project.join("raw/unused.md"), "# raw\n").expect("raw");
        fs::write(project.join("output/pdf/unused.pdf"), b"pdf").expect("pdf");
        fs::write(project.join("tmp/unused.png"), b"tmp").expect("tmp");

        let graph = dependency_graph(
            &project,
            &MergedConfig {
                entry: Some("main.md".to_string()),
                metadata_file: Some("styles/book-metadata.yaml".to_string()),
                pandoc_css: Some("styles/book.css".to_string()),
                pandoc_epub_css: Some("styles/book.css".to_string()),
                ..Default::default()
            },
        );

        for expected in [
            ".omnidoc.toml",
            "main.md",
            "chapters/chapter.md",
            "chapters/nested/part.md",
            "styles/book-metadata.yaml",
            "styles/book.css",
            "styles/theme.tex",
            "assets/used.png",
            "assets/cover.png",
        ] {
            assert!(
                graph.files.contains(&expected.to_string()),
                "missing {expected}"
            );
        }
        assert!(!graph.files.iter().any(|path| path.starts_with("raw/")));
        assert!(!graph.files.iter().any(|path| path.starts_with("output/")));
        assert!(!graph.files.iter().any(|path| path.starts_with("tmp/")));

        fs::remove_dir_all(project).expect("cleanup");
    }

    #[test]
    fn pdf_dependency_graph_tracks_pre_rendered_svg_siblings() {
        let project = tempfile::tempdir().expect("project tempdir");
        fs::create_dir_all(project.path().join("assets")).expect("assets dir");
        fs::write(
            project.path().join(".omnidoc.toml"),
            "[project]\nentry='main.md'\n",
        )
        .expect("config");
        fs::write(
            project.path().join("main.md"),
            "# Book\n\n![diagram](assets/diagram.svg)\n",
        )
        .expect("entry");
        fs::write(project.path().join("assets/diagram.svg"), b"<svg/>").expect("svg");
        fs::write(project.path().join("assets/diagram.pdf"), b"pdf").expect("pdf");

        let pdf_graph = dependency_graph(
            project.path(),
            &MergedConfig {
                entry: Some("main.md".to_string()),
                to: Some("pdf".to_string()),
                ..Default::default()
            },
        );
        assert!(pdf_graph.files.contains(&"assets/diagram.pdf".to_string()));

        let html_graph = dependency_graph(
            project.path(),
            &MergedConfig {
                entry: Some("main.md".to_string()),
                to: Some("html".to_string()),
                ..Default::default()
            },
        );
        assert!(!html_graph.files.contains(&"assets/diagram.pdf".to_string()));
    }

    #[test]
    fn dependency_graph_uses_filter_depfiles_for_actual_includes() {
        let project = tempfile::tempdir().expect("project tempdir");
        let library = tempfile::tempdir().expect("library tempdir");
        let external = tempfile::NamedTempFile::new().expect("external include");
        fs::write(
            project.path().join(".omnidoc.toml"),
            "[project]\nentry='main.md'\n",
        )
        .expect("config");
        fs::write(project.path().join("main.md"), "# Main\n").expect("entry");
        fs::create_dir_all(project.path().join("chapters")).expect("chapters");
        let chapter = project.path().join("chapters/actual.md");
        fs::write(&chapter, "# Actual include\n").expect("included chapter");
        fs::create_dir_all(project.path().join(CACHE_DIR)).expect("cache dir");
        fs::write(
            project.path().join(CACHE_DIR).join(INCLUDE_DEPFILE),
            format!(
                "# omnidoc-depfile-v1\n{}\n{}\n",
                chapter.display(),
                external.path().display()
            ),
        )
        .expect("depfile");

        let graph = dependency_graph(
            project.path(),
            &MergedConfig {
                entry: Some("main.md".to_string()),
                to: Some("html".to_string()),
                lib_path: Some(library.path().to_string_lossy().to_string()),
                ..Default::default()
            },
        );

        assert!(graph.files.contains(&"chapters/actual.md".to_string()));
        assert!(graph.resources.iter().any(|resource| {
            resource.logical_name == format!("include-depfile:{}", INCLUDE_DEPFILE)
                && resource.resolved_from == "external"
                && resource.path == external.path().to_string_lossy()
        }));

        fs::write(
            project.path().join(CACHE_DIR).join(INCLUDE_DEPFILE),
            format!("# unknown-depfile\n{}\n", chapter.display()),
        )
        .expect("invalid depfile");
        let ignored = dependency_graph(
            project.path(),
            &MergedConfig {
                entry: Some("main.md".to_string()),
                to: Some("html".to_string()),
                lib_path: Some(library.path().to_string_lossy().to_string()),
                ..Default::default()
            },
        );
        assert!(!ignored.files.contains(&"chapters/actual.md".to_string()));
    }

    #[test]
    fn shared_resources_invalidate_cache_and_lock_uses_portable_digests() {
        let project = tempfile::tempdir().expect("project tempdir");
        let library = tempfile::tempdir().expect("library tempdir");
        fs::write(
            library.path().join("manifest.toml"),
            "manifest_version = 1\nversion = '1.0.0'\nchecksum_file = 'checksums.sha256'\n",
        )
        .expect("library manifest");
        fs::write(library.path().join("checksums.sha256"), "abc  payload\n").expect("checksums");
        let css_dir = library.path().join("pandoc/css");
        fs::create_dir_all(&css_dir).expect("css dir");
        fs::write(
            css_dir.join("omnidoc-base.css"),
            ".omni-display-math { text-align: center; }\n",
        )
        .expect("base css");
        let filter_dir = library.path().join("pandoc/data/filters");
        fs::create_dir_all(&filter_dir).expect("filter dir");
        fs::write(filter_dir.join("include-files.lua"), "return {}\n").expect("html filter");
        fs::write(filter_dir.join("display-math.lua"), "return {}\n").expect("display math filter");
        fs::write(filter_dir.join("ltblr.lua"), "return {}\n").expect("latex filter");
        let texmf = library.path().join("texmf/tex/latex");
        fs::create_dir_all(&texmf).expect("texmf dir");
        fs::write(texmf.join("theme.sty"), "% theme\n").expect("style");
        let css = css_dir.join("engineering-book.css");
        fs::write(&css, "body { color: black; }\n").expect("css");
        fs::write(
            project.path().join(".omnidoc.toml"),
            "[project]\nentry='main.md'\n",
        )
        .expect("config");
        fs::write(project.path().join("main.md"), "# Book\n").expect("entry");
        let config = MergedConfig {
            entry: Some("main.md".to_string()),
            to: Some("html".to_string()),
            lib_path: Some(library.path().to_string_lossy().to_string()),
            pandoc_css: Some("engineering-book.css".to_string()),
            ..Default::default()
        };

        let graph = dependency_graph(project.path(), &config);
        assert!(graph.resources.iter().any(|resource| {
            resource.logical_name == "html-css"
                && resource.resolved_from == "omnidoc-libs"
                && resource.path == css.to_string_lossy()
        }));
        assert!(graph
            .resources
            .iter()
            .any(|resource| resource.logical_name == "lua-filter:include-files.lua"));
        assert!(graph
            .resources
            .iter()
            .any(|resource| resource.logical_name == "lua-filter:display-math.lua"));
        assert!(graph
            .resources
            .iter()
            .any(|resource| resource.logical_name == "omnidoc-base-css"));
        assert!(graph
            .resources
            .iter()
            .any(|resource| resource.logical_name == "omnidoc-libs-manifest"));
        assert!(graph
            .resources
            .iter()
            .any(|resource| resource.logical_name == "omnidoc-libs-checksums"));
        assert!(!graph
            .resources
            .iter()
            .any(|resource| resource.logical_name == "lua-filter:ltblr.lua"));
        assert!(!graph
            .resources
            .iter()
            .any(|resource| resource.logical_name == "texmf"));

        let before =
            build_input_digest(project.path(), &graph, &config, "html").expect("initial digest");
        write_cache(project.path(), "html", &before).expect("cache");
        assert!(cache_hit(project.path(), "html", &before));

        fs::write(&css, "body { color: navy; }\n").expect("updated css");
        let after =
            build_input_digest(project.path(), &graph, &config, "html").expect("updated digest");
        assert_ne!(before, after);
        assert!(!cache_hit(project.path(), "html", &after));

        write_lock(project.path(), &config, &graph).expect("lock");
        let lock_text = fs::read_to_string(project.path().join("omnidoc.lock")).expect("lock text");
        let lock: LockFile = toml::from_str(&lock_text).expect("lock v4");
        assert_eq!(lock.lock_version, 4);
        let locked_library = lock.library.as_ref().expect("locked library");
        assert_eq!(locked_library.version.as_deref(), Some("1.0.0"));
        assert!(locked_library
            .manifest_digest
            .as_deref()
            .is_some_and(|digest| digest.starts_with("blake3:")));
        assert!(locked_library
            .checksums_digest
            .as_deref()
            .is_some_and(|digest| digest.starts_with("blake3:")));
        let html = lock.targets.get("html").expect("html target");
        assert!(html.input_digest.starts_with("blake3:"));
        assert!(html.resources.iter().any(|resource| {
            resource.logical_name == "html-css"
                && resource.resolved_from == "omnidoc-libs"
                && resource.digest.starts_with("blake3:")
        }));
        assert!(!lock_text.contains(&library.path().to_string_lossy().to_string()));
    }

    #[test]
    fn build_report_records_cache_timing_toolchain_and_artifact_digest() {
        let project = tempfile::tempdir().expect("project");
        let artifact = project.path().join("book.html");
        fs::write(&artifact, "<h1>Book</h1>\n").expect("artifact");
        let graph = super::DependencyGraph {
            files: vec!["main.md".to_string()],
            resources: Vec::new(),
        };

        let config = MergedConfig::default();
        let report = build_report(super::BuildReportContext {
            output: "html".to_string(),
            target: "book".to_string(),
            skipped: true,
            cache_reason: "input_digest_match".to_string(),
            duration_ms: 12,
            input_digest: "blake3:input".to_string(),
            graph: &graph,
            config: &config,
            artifact: &artifact,
            compatibility: None,
            issues: Vec::new(),
        });

        assert_eq!(report.cache_reason, "input_digest_match");
        assert_eq!(report.duration_ms, 12);
        assert!(report
            .artifact_digest
            .as_deref()
            .is_some_and(|digest| digest.starts_with("blake3:")));
        assert!(report.toolchain.contains_key("pandoc"));
        assert!(!report.toolchain.contains_key("latex_engine"));
    }

    #[test]
    fn lock_v4_keeps_multiple_output_targets() {
        let project = tempfile::tempdir().expect("project");
        fs::write(project.path().join("main.md"), "# Book\n").expect("entry");
        let html_config = MergedConfig {
            entry: Some("main.md".to_string()),
            to: Some("html".to_string()),
            ..Default::default()
        };
        let epub_config = MergedConfig {
            entry: Some("main.md".to_string()),
            to: Some("epub".to_string()),
            ..Default::default()
        };
        let html_graph = dependency_graph(project.path(), &html_config);
        let epub_graph = dependency_graph(project.path(), &epub_config);

        write_lock_targets(
            project.path(),
            &[
                LockTargetInput {
                    output: "html",
                    config: &html_config,
                    graph: &html_graph,
                },
                LockTargetInput {
                    output: "epub",
                    config: &epub_config,
                    graph: &epub_graph,
                },
            ],
        )
        .expect("multi-target lock");

        let lock_text = fs::read_to_string(project.path().join("omnidoc.lock")).expect("lock");
        let lock: LockFile = toml::from_str(&lock_text).expect("lock v4");
        assert_eq!(lock.lock_version, 4);
        assert!(!lock.toolchain.contains_key("latex_engine"));
        assert_eq!(
            lock.targets.keys().cloned().collect::<Vec<_>>(),
            ["epub", "html"]
        );
        assert_ne!(
            lock.targets["html"].input_digest,
            lock.targets["epub"].input_digest
        );
    }

    #[test]
    fn validates_unsupported_build_outputs() {
        let config = MergedConfig {
            outputs: vec!["pdf".to_string(), "unknown".to_string()],
            ..Default::default()
        };
        let issues = validate_config(Path::new("."), &config);

        assert!(issues
            .iter()
            .any(|issue| issue.message.contains("build.outputs")));
    }

    #[test]
    fn validates_pandoc_format_option_keys() {
        let config = MergedConfig {
            pandoc_format_options: std::collections::BTreeMap::from([(
                "html5".to_string(),
                vec!["--toc-depth=3".to_string()],
            )]),
            ..Default::default()
        };

        let issues = validate_config(Path::new("."), &config);

        assert!(issues
            .iter()
            .any(|issue| issue.message.contains("pandoc.format_options")));
    }

    #[test]
    fn validates_css_names_resolved_from_omnidoc_libs() {
        let project = temporary_project("shared-css-project");
        let library = temporary_project("shared-css-library");
        fs::create_dir_all(library.join("pandoc/css")).expect("css dir");
        fs::create_dir_all(&project).expect("project dir");
        fs::write(project.join("main.md"), "# Book\n").expect("entry");
        fs::write(
            library.join("pandoc/css/engineering-book.css"),
            "body { max-width: 56rem; }\n",
        )
        .expect("shared css");
        let issues = validate_config(
            &project,
            &MergedConfig {
                entry: Some("main.md".to_string()),
                lib_path: Some(library.to_string_lossy().to_string()),
                pandoc_css: Some("engineering-book.css".to_string()),
                pandoc_epub_css: Some("engineering-book.css".to_string()),
                ..Default::default()
            },
        );
        assert!(!issues
            .iter()
            .any(|issue| issue.message.contains("pandoc.css not found")));
        assert!(!issues
            .iter()
            .any(|issue| issue.message.contains("pandoc.epub_css not found")));

        fs::remove_dir_all(project).expect("project cleanup");
        fs::remove_dir_all(library).expect("library cleanup");
    }

    #[test]
    fn validates_engine_pass_count() {
        let config = MergedConfig {
            latex_backend: "engine".to_string(),
            max_latex_passes: 0,
            ..Default::default()
        };
        let issues = validate_config(Path::new("."), &config);

        assert!(issues
            .iter()
            .any(|issue| issue.message.contains("max_latex_passes")));
    }

    #[test]
    fn parses_hook_command_arguments() {
        assert_eq!(
            hook_argv(&HookCommand::String("scripts/pre arg".to_string())),
            vec!["scripts/pre", "arg"]
        );
        assert_eq!(
            hook_argv(&HookCommand::Args(vec![
                "tool".to_string(),
                "--flag".to_string()
            ])),
            vec!["tool", "--flag"]
        );
    }

    #[test]
    fn lists_and_validates_hook_metadata() {
        let manifest = PluginManifest {
            key: Some("sample".to_string()),
            name: None,
            version: None,
            description: None,
            kind: None,
            language: None,
            template_file: None,
            hooks: Some(PluginHooks {
                pre_build: Some(HookCommand::Args(vec!["tool".to_string()])),
                post_build: None,
                lint_rule: Some(HookCommand::String("lint-tool".to_string())),
                asset_provider: None,
            }),
        };

        assert_eq!(
            manifest_hook_names(&manifest),
            vec!["pre_build".to_string(), "lint_rule".to_string()]
        );
        assert!(validate_hook_command(
            Path::new("plugins/sample/manifest.toml"),
            &HookCommand::String("tool".to_string())
        )
        .is_none());
        assert!(validate_hook_command(
            Path::new("plugins/sample/manifest.toml"),
            &HookCommand::String("scripts/missing.sh".to_string())
        )
        .is_some());
    }

    #[test]
    fn parses_plugin_lint_rule_output() {
        let plugin = LoadedPlugin {
            info: PluginInfo {
                path: "plugins/sample/manifest.toml".to_string(),
                key: "sample".to_string(),
                name: None,
                version: None,
                description: None,
                kind: "plugin".to_string(),
                hooks: Vec::new(),
                valid: true,
                error: None,
            },
            manifest: PluginManifest {
                key: Some("sample".to_string()),
                name: None,
                version: None,
                description: None,
                kind: None,
                language: None,
                template_file: None,
                hooks: None,
            },
            base_dir: PathBuf::from("plugins/sample"),
        };

        let issues = parse_lint_rule_output(
            &plugin,
            "warning:main.md:7:3:custom lint warning\ninfo:main.md:9:1:note",
        );

        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0].path.as_deref(), Some("main.md"));
        assert_eq!(issues[0].line, Some(7));
        assert!(issues[0].message.contains("column 3"));
    }

    #[cfg(unix)]
    #[test]
    fn executes_plugin_hook_command() {
        use super::{run_hook_command_capture, PluginContext, PluginHook};
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        use std::time::{SystemTime, UNIX_EPOCH};

        let root = std::env::temp_dir().join(format!(
            "omnidoc-hook-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        let plugin_dir = root.join("plugin");
        fs::create_dir_all(&plugin_dir).expect("plugin dir");
        let script = plugin_dir.join("hook.sh");
        fs::write(
            &script,
            "#!/bin/sh\nprintf '%s:%s' \"$OMNIDOC_HOOK\" \"$OMNIDOC_OUTPUT\"\n",
        )
        .expect("script");
        let mut permissions = fs::metadata(&script).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script, permissions).expect("permissions");

        let plugin = LoadedPlugin {
            info: PluginInfo {
                path: plugin_dir.join("manifest.toml").display().to_string(),
                key: "hook-test".to_string(),
                name: None,
                version: None,
                description: None,
                kind: "plugin".to_string(),
                hooks: Vec::new(),
                valid: true,
                error: None,
            },
            manifest: PluginManifest {
                key: Some("hook-test".to_string()),
                name: None,
                version: None,
                description: None,
                kind: None,
                language: None,
                template_file: None,
                hooks: None,
            },
            base_dir: plugin_dir,
        };
        let config = MergedConfig::default();
        let context = PluginContext {
            project_path: &root,
            config: &config,
            output: Some("html"),
            target: Some("manual"),
        };

        let output = run_hook_command_capture(
            &context,
            &plugin,
            &HookCommand::Args(vec!["hook.sh".to_string()]),
            PluginHook::PreBuild,
        )
        .expect("hook output");

        assert_eq!(output, "pre_build:html");
        let _ = fs::remove_dir_all(root);
    }
}
