use crate::build::executor::{BuildExecutor, LatexEnginePreference};
use crate::build::pandoc_policy::{is_supported_format_key, PandocOutputKind};
use crate::build::pipeline::{detect_project_type, ProjectType};
use crate::build::tectonic;
use crate::cli::handlers::theme::{font_family_matches, valid_latex_package_name};
use crate::config::MergedConfig;
use crate::constants::pandoc;
use crate::epub::{is_supported_epub_profile, EpubCompatibilityReport};
use crate::error::{OmniDocError, Result};
use crate::extensions::{
    enabled_plugin_resources, enabled_plugins, materialize_theme_tokens, plugin_filters_for_output,
    resolve_selected_theme, PackageKind, ResolvedTheme,
};
use crate::utils;
use crate::utils::directories::data_local_dir;
use blake3::Hasher;
use fs2::FileExt;
use percent_encoding::percent_decode_str;
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
pub(crate) const LATEX_INPUT_DEPFILE: &str = "latex-inputs.d";
const LOCK_FILE: &str = "omnidoc.lock";
const REPORT_FILE: &str = "omnidoc-report.json";
const PROJECT_LOCK_FILE: &str = "project.lock";
const CACHE_VERSION: u32 = 7;
const LOCK_VERSION: u32 = 5;

pub struct ProjectWriteLock {
    file: fs::File,
}

impl Drop for ProjectWriteLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

pub fn acquire_project_write_lock(
    project_path: &Path,
    operation: &str,
) -> Result<ProjectWriteLock> {
    let cache_dir = project_path.join(CACHE_DIR);
    fs::create_dir_all(&cache_dir)?;
    let path = cache_dir.join(PROJECT_LOCK_FILE);
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&path)?;
    file.try_lock_exclusive().map_err(|error| {
        OmniDocError::Project(format!(
            "cannot {operation}: another OmniDoc process holds {} ({error})",
            path.display()
        ))
    })?;
    Ok(ProjectWriteLock { file })
}

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
    pub cache_details: Vec<String>,
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
    pub project_path: &'a Path,
    pub output: String,
    pub target: String,
    pub skipped: bool,
    pub cache_reason: String,
    pub cache_details: Vec<String>,
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
    #[serde(default)]
    components: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct BuildInputState {
    pub input_digest: String,
    pub components: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct CacheProbe {
    pub hit: bool,
    pub details: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockFile {
    pub lock_version: u32,
    pub omnidoc_version: String,
    pub library: Option<LockedLibrary>,
    #[serde(default)]
    pub packages: Vec<LockedPackage>,
    pub toolchain: BTreeMap<String, String>,
    pub targets: BTreeMap<String, LockedTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct LockedPackage {
    pub kind: PackageKind,
    pub id: String,
    pub version: String,
    pub source: String,
    pub digest: String,
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
    pub packages_up_to_date: bool,
    pub toolchain_up_to_date: bool,
    pub missing_packages: Vec<LockedPackage>,
    pub extra_packages: Vec<LockedPackage>,
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

pub fn supported_outputs() -> &'static [&'static str] {
    &["pdf", "html", "epub", "docx", "pptx", "latex"]
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
                    "Unsupported pandoc.format_options key '{}'. Supported keys: pdf, html, epub, docx, pptx, latex",
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

    if let Some(bundle) = config.tectonic_bundle.as_deref() {
        if !bundle.contains("://") {
            let expanded = tectonic::expand_home(bundle);
            check_configured_path(
                project_path,
                &expanded,
                "Configured tectonic.bundle not found",
                true,
                &mut issues,
            );
        }
    }
    for search_path in &config.tectonic_search_paths {
        let normalized = search_path
            .trim()
            .trim_end_matches("//")
            .trim_end_matches("\\\\");
        if !normalized.is_empty() {
            let expanded = tectonic::expand_home(normalized);
            check_configured_path(
                project_path,
                &expanded,
                "Configured tectonic.search_paths directory not found",
                true,
                &mut issues,
            );
        }
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
        if let Err(theme_error) = resolve_selected_theme(Some(project_path), config) {
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

    if !config.plugins_enabled.is_empty() {
        if let Err(plugin_error) = enabled_plugins(project_path, config) {
            issues.push(error(
                format!("Invalid plugin configuration: {plugin_error}"),
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

    if let Some(reference_doc) = &config.pandoc_pptx_reference_doc {
        check_configured_path(
            project_path,
            reference_doc,
            "Configured pandoc.pptx_reference_doc not found",
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
    let latex_resource_re = regex::Regex::new(
        r#"\\(?P<command>includegraphics|input|include|bibliography|addbibresource)(?:\s*\[[^]]*\])?\s*\{(?P<target>[^}]+)\}"#,
    )
    .expect("LaTeX resource regex");

    for file in source_files(project_path) {
        let Ok(content) = fs::read_to_string(&file) else {
            continue;
        };
        let rel = display_relative(project_path, &file);
        let extension = file
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let is_markdown = matches!(extension.as_str(), "md" | "markdown");
        let is_latex = matches!(extension.as_str(), "tex" | "sty" | "cls");
        let mut markdown_fence: Option<char> = None;
        let mut latex_verbatim: Option<String> = None;

        for (line_index, line) in content.lines().enumerate() {
            let line_no = line_index + 1;

            if is_markdown {
                let trimmed = line.trim_start();
                if let Some(marker) = markdown_fence {
                    if trimmed.starts_with(&marker.to_string().repeat(3)) {
                        markdown_fence = None;
                    }
                    continue;
                }
                if trimmed.starts_with("```") {
                    markdown_fence = Some('`');
                    continue;
                }
                if trimmed.starts_with("~~~") {
                    markdown_fence = Some('~');
                    continue;
                }
                if line.starts_with("    ") || line.starts_with('\t') {
                    continue;
                }

                for capture in image_re.captures_iter(line) {
                    check_local_target(
                        project_path,
                        &file,
                        &capture[1],
                        &rel,
                        line_no,
                        &mut issues,
                    );
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
                    check_local_target(
                        project_path,
                        &file,
                        &capture[1],
                        &rel,
                        line_no,
                        &mut issues,
                    );
                }
                continue;
            }

            if is_latex {
                if let Some(environment) = latex_verbatim.as_deref() {
                    if line.contains(&format!(r"\end{{{environment}}}")) {
                        latex_verbatim = None;
                    }
                    continue;
                }
                for environment in ["verbatim", "Verbatim", "lstlisting", "minted"] {
                    if line.contains(&format!(r"\begin{{{environment}}}")) {
                        latex_verbatim = Some(environment.to_string());
                        break;
                    }
                }
                if latex_verbatim.is_some() {
                    continue;
                }

                let source = strip_latex_comment(line);
                for capture in latex_resource_re.captures_iter(source) {
                    let command = capture
                        .name("command")
                        .map(|value| value.as_str())
                        .unwrap_or("");
                    let target = capture
                        .name("target")
                        .map(|value| value.as_str())
                        .unwrap_or("");
                    for target in target
                        .split(',')
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                    {
                        check_latex_target(
                            project_path,
                            &file,
                            target,
                            command,
                            &rel,
                            line_no,
                            &mut issues,
                        );
                    }
                }
            }
        }
    }

    issues
}

fn strip_latex_comment(line: &str) -> &str {
    let mut escaped = false;
    for (index, character) in line.char_indices() {
        if character == '%' && !escaped {
            return &line[..index];
        }
        if character == '\\' {
            escaped = !escaped;
        } else {
            escaped = false;
        }
    }
    line
}

fn check_latex_target(
    project_path: &Path,
    source_file: &Path,
    target: &str,
    command: &str,
    rel: &str,
    line: usize,
    issues: &mut Vec<ProjectIssue>,
) {
    if target.contains(['\\', '#']) || !is_local_path(target) {
        return;
    }

    let base = source_file.parent().unwrap_or(project_path);
    let path = Path::new(target);
    let mut candidates = vec![path.to_path_buf()];
    if path.extension().is_none() {
        let extensions: &[&str] = match command {
            "includegraphics" => &["pdf", "png", "jpg", "jpeg", "svg", "eps"],
            "bibliography" | "addbibresource" => &["bib"],
            "input" | "include" => &["tex"],
            _ => &[],
        };
        candidates.extend(
            extensions
                .iter()
                .map(|extension| path.with_extension(extension)),
        );
    }

    if candidates.iter().any(|candidate| {
        base.join(candidate).exists()
            || project_path.join(candidate).exists()
            || project_path.join("tex").join(candidate).exists()
    }) {
        return;
    }

    issues.push(warning(
        format!("Referenced local resource not found: {target}"),
        Some(rel.to_string()),
        Some(line),
    ));
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
        config.pandoc_pptx_reference_doc.as_ref(),
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
    let library_root = omnidoc_library_root(config);
    if let Some(bundle) = config
        .tectonic_bundle
        .as_deref()
        .filter(|bundle| !bundle.contains("://"))
    {
        let resolved = tectonic::resolve_bundle(project_path, bundle);
        if resolved.is_file() {
            let canonical_project = project_path
                .canonicalize()
                .unwrap_or_else(|_| project_path.to_path_buf());
            let canonical_bundle = resolved.canonicalize().unwrap_or(resolved);
            if canonical_bundle.starts_with(&canonical_project) {
                track_dependency(
                    project_path,
                    project_path,
                    &canonical_bundle,
                    &mut files,
                    &mut pending,
                );
            } else {
                add_resolved_resource(
                    &mut depfile_resources,
                    project_path,
                    &library_root,
                    "tectonic-bundle".to_string(),
                    canonical_bundle,
                );
            }
        }
    }
    let mut effective_pandoc_options = config.pandoc_options.clone();
    if let Some(options) = config.pandoc_format_options.get(output_kind.config_key()) {
        effective_pandoc_options.extend(options.clone());
    }
    let canonical_project = project_path
        .canonicalize()
        .unwrap_or_else(|_| project_path.to_path_buf());
    for (flag, configured) in pandoc_option_file_references(&effective_pandoc_options) {
        let candidate = PathBuf::from(&configured);
        if candidate.is_absolute() {
            if let Ok(canonical) = candidate.canonicalize() {
                if canonical.is_file() && canonical.starts_with(&canonical_project) {
                    track_dependency(
                        project_path,
                        project_path,
                        Path::new(&display_relative(&canonical_project, &canonical)),
                        &mut files,
                        &mut pending,
                    );
                } else if canonical.is_file() {
                    add_resolved_resource(
                        &mut depfile_resources,
                        project_path,
                        &library_root,
                        format!(
                            "pandoc-option:{}:{}",
                            flag.trim_start_matches('-'),
                            portable_external_resource_name(&canonical)
                        ),
                        canonical,
                    );
                }
            }
        } else if is_local_path(&configured) {
            track_dependency(
                project_path,
                project_path,
                &candidate,
                &mut files,
                &mut pending,
            );
        }
    }
    let mut depfiles = output_kind
        .filters(config)
        .into_iter()
        .filter_map(filter_depfile_name)
        .collect::<BTreeSet<_>>();
    if let Ok(plugin_filters) =
        plugin_filters_for_output(project_path, config, output_kind.config_key())
    {
        depfiles.extend(
            plugin_filters
                .into_iter()
                .filter_map(|filter| filter.depfile_name()),
        );
    }
    for depfile in depfiles {
        load_depfile_dependencies(
            project_path,
            &library_root,
            &depfile,
            &format!("filter-depfile:{depfile}"),
            &mut files,
            &mut pending,
            &mut depfile_resources,
        );
    }
    if output_kind == PandocOutputKind::Pdf {
        load_depfile_dependencies(
            project_path,
            &library_root,
            LATEX_INPUT_DEPFILE,
            "latex-fls-input",
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

pub(crate) fn filter_depfile_name(filter: &str) -> Option<String> {
    let stem = Path::new(filter).file_stem()?.to_str()?;
    let normalized = stem
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    (!normalized.is_empty()).then(|| format!("{normalized}.d"))
}

pub(crate) fn filter_depfile_metadata_key(filter: &str) -> Option<String> {
    filter_depfile_name(filter)
        .and_then(|name| name.strip_suffix(".d").map(str::to_string))
        .map(|stem| format!("omnidoc-depfile-{stem}"))
}

fn pandoc_option_file_references(options: &[String]) -> Vec<(String, String)> {
    const FILE_FLAGS: &[&str] = &[
        "--bibliography",
        "--csl",
        "--css",
        "--epub-cover-image",
        "--include-after-body",
        "--include-before-body",
        "--include-in-header",
        "--lua-filter",
        "--metadata-file",
        "--reference-doc",
        "--template",
        "-A",
        "-B",
        "-H",
    ];
    let mut references = Vec::new();
    let mut index = 0;
    while index < options.len() {
        let option = &options[index];
        if let Some((flag, value)) = option.split_once('=') {
            if FILE_FLAGS.contains(&flag) && !value.trim().is_empty() {
                references.push((flag.to_string(), value.to_string()));
            }
        } else if FILE_FLAGS.contains(&option.as_str()) {
            if let Some(value) = options
                .get(index + 1)
                .filter(|value| !value.trim().is_empty())
            {
                references.push((option.clone(), value.clone()));
                index += 1;
            }
        }
        index += 1;
    }
    references
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
    library_root: &Path,
    depfile_name: &str,
    logical_name: &str,
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
            let resource_name = if logical_name == "latex-fls-input" {
                format!(
                    "latex-fls-input:{}",
                    portable_external_resource_name(&canonical)
                )
            } else {
                logical_name.to_string()
            };
            add_resolved_resource(
                external,
                project_path,
                library_root,
                resource_name,
                canonical,
            );
        }
    }
}

fn portable_external_resource_name(path: &Path) -> String {
    let components = path
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect::<Vec<_>>();
    if let Some(index) = components
        .iter()
        .rposition(|component| matches!(*component, "texmf-dist" | "texmf" | "fonts"))
    {
        return components[index..].join("/");
    }
    components[components.len().saturating_sub(5)..].join("/")
}

fn resolved_build_resources(project_path: &Path, config: &MergedConfig) -> Vec<ResolvedResource> {
    let library_root = omnidoc_library_root(config);
    let mut resources = BTreeMap::<String, ResolvedResource>::new();
    let output_kind = PandocOutputKind::from_config(config).unwrap_or(PandocOutputKind::Pdf);
    let theme = resolve_selected_theme(Some(project_path), config)
        .ok()
        .flatten()
        .filter(|theme| theme.supports_output(output_kind.config_key()));

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
    if let Some(path) = existing_path(library_root.join(".omnidoc-release.toml")) {
        add_resolved_resource(
            &mut resources,
            project_path,
            &library_root,
            "omnidoc-libs-release".to_string(),
            path,
        );
    }

    if let Some(theme) = &theme {
        for package in &theme.packages {
            for path in &package.tracked_files {
                let relative = path
                    .strip_prefix(&package.root)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .replace('\\', "/");
                add_resolved_resource(
                    &mut resources,
                    project_path,
                    &library_root,
                    format!(
                        "theme-package:{}@{}:{}",
                        package.id, package.version, relative
                    ),
                    path.clone(),
                );
            }
        }
        if let Ok(generated) = materialize_theme_tokens(theme, project_path) {
            if let Some(path) = generated.css {
                add_resolved_resource(
                    &mut resources,
                    project_path,
                    &library_root,
                    format!("theme-generated-css:{}@{}", theme.id, theme.version),
                    path,
                );
            }
            if let Some(path) = generated.latex_header {
                add_resolved_resource(
                    &mut resources,
                    project_path,
                    &library_root,
                    format!("theme-generated-latex:{}@{}", theme.id, theme.version),
                    path,
                );
            }
        }
    }

    if let Ok(plugin_resources) = enabled_plugin_resources(project_path, config) {
        for resource in plugin_resources {
            insert_resolved_resource(
                &mut resources,
                resource.logical_name,
                resource.resolved_from,
                resource.path,
            );
        }
    }

    for filter in output_kind.filters(config) {
        // Keep this resolution identical to PandocBuilder::push_lua_filters:
        // filter names are relative to the shared filter directory.
        if let Some(path) =
            existing_path(library_root.join(pandoc::LIB_PANDOC_FILTERS).join(filter))
        {
            add_resolved_resource(
                &mut resources,
                project_path,
                &library_root,
                format!("lua-filter:{filter}"),
                path,
            );
        }
    }
    if output_kind.uses_latex_defaults() {
        for (filter, relative) in [
            ("emoji.lua", pandoc::LIB_PANDOC_HEADER_EMOJI),
            ("admonition.lua", pandoc::LIB_PANDOC_HEADER_SEMANTIC_BLOCKS),
        ] {
            if output_kind.filters(config).contains(&filter) {
                if let Some(path) = existing_path(library_root.join(relative)) {
                    add_resolved_resource(
                        &mut resources,
                        project_path,
                        &library_root,
                        format!("omnidoc-latex-header:{relative}"),
                        path,
                    );
                }
            }
        }
        if let Some(theme) = &theme {
            for header in &theme.resources.latex_headers {
                if let Some(path) = existing_path(header.clone()) {
                    let logical_name =
                        theme_resource_logical_name(theme, "theme-latex-header", &path);
                    add_resolved_resource(
                        &mut resources,
                        project_path,
                        &library_root,
                        logical_name,
                        path,
                    );
                }
            }
            for package in &theme.resources.latex_packages {
                if let Some(path) = existing_path(package.clone()) {
                    let logical_name =
                        theme_resource_logical_name(theme, "theme-latex-package", &path);
                    add_resolved_resource(
                        &mut resources,
                        project_path,
                        &library_root,
                        logical_name,
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

    let (configured_css, theme_css, fallback_css, logical_prefix) = match output_kind {
        PandocOutputKind::Html => (
            config.pandoc_css.as_deref(),
            theme
                .as_ref()
                .map(|theme| theme.resources.html_css.as_slice()),
            Some(pandoc::LIB_PANDOC_CSS_DEFAULT),
            "html-css",
        ),
        PandocOutputKind::Epub => (
            config
                .pandoc_epub_css
                .as_deref()
                .or(config.pandoc_css.as_deref()),
            theme
                .as_ref()
                .map(|theme| theme.resources.epub_css.as_slice()),
            Some("pandoc/data/epub.css"),
            "epub-css",
        ),
        _ => (None, None, None, "css"),
    };
    if let Some(configured_css) = configured_css {
        if let Some(path) = resolve_resource_path(
            project_path,
            &library_root,
            configured_css,
            Some("pandoc/css"),
        ) {
            add_resolved_resource(
                &mut resources,
                project_path,
                &library_root,
                logical_prefix.to_string(),
                path,
            );
        }
    } else if let Some(theme_css) = theme_css.filter(|resources| !resources.is_empty()) {
        for (index, css) in theme_css.iter().enumerate() {
            if let Some(path) = existing_path(css.clone()) {
                add_resolved_resource(
                    &mut resources,
                    project_path,
                    &library_root,
                    format!("{logical_prefix}-{}", index + 1),
                    path,
                );
            }
        }
    } else if let Some(fallback_css) = fallback_css {
        if let Some(path) = existing_path(library_root.join(fallback_css)) {
            add_resolved_resource(
                &mut resources,
                project_path,
                &library_root,
                logical_prefix.to_string(),
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
            theme
                .as_ref()
                .and_then(|theme| theme.resources.latex_template.as_deref()),
        )),
        PandocOutputKind::Html => Some((
            "html-template",
            config
                .pandoc_html_template
                .as_deref()
                .or(config.pandoc_template.as_deref()),
            theme
                .as_ref()
                .and_then(|theme| theme.resources.html_template.as_deref()),
        )),
        PandocOutputKind::Epub => Some((
            "epub-template",
            config
                .pandoc_epub_template
                .as_deref()
                .or(config.pandoc_template.as_deref()),
            theme
                .as_ref()
                .and_then(|theme| theme.resources.epub_template.as_deref()),
        )),
        PandocOutputKind::Docx | PandocOutputKind::Pptx => None,
    };
    if let Some((logical_name, configured, themed)) = template {
        let selected = if let Some(configured) = configured {
            resolve_resource_path(
                project_path,
                &library_root,
                configured,
                Some("pandoc/data/templates"),
            )
        } else {
            themed.and_then(|path| existing_path(path.to_path_buf()))
        };
        if let Some(path) = selected {
            add_resolved_resource(
                &mut resources,
                project_path,
                &library_root,
                logical_name.to_string(),
                path,
            );
        }
    }

    if output_kind == PandocOutputKind::Docx {
        let reference_doc = if let Some(configured) = config.pandoc_reference_doc.as_deref() {
            resolve_resource_path(project_path, &library_root, configured, None)
        } else {
            theme
                .as_ref()
                .and_then(|theme| theme.resources.docx_reference_doc.clone())
                .and_then(existing_path)
        };
        if let Some(path) = reference_doc {
            add_resolved_resource(
                &mut resources,
                project_path,
                &library_root,
                "reference-doc".to_string(),
                path,
            );
        }
    }

    if output_kind == PandocOutputKind::Pptx {
        let configured = config
            .pandoc_pptx_reference_doc
            .as_deref()
            .or(config.pandoc_reference_doc.as_deref());
        let reference_doc = if let Some(configured) = configured {
            resolve_resource_path(project_path, &library_root, configured, None)
        } else {
            theme
                .as_ref()
                .and_then(|theme| theme.resources.pptx_reference_doc.clone())
                .and_then(existing_path)
        };
        if let Some(path) = reference_doc {
            add_resolved_resource(
                &mut resources,
                project_path,
                &library_root,
                "pptx-reference-doc".to_string(),
                path,
            );
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

fn insert_resolved_resource(
    resources: &mut BTreeMap<String, ResolvedResource>,
    logical_name: String,
    resolved_from: String,
    path: PathBuf,
) {
    let path = path.canonicalize().unwrap_or(path);
    let key = format!("{logical_name}:{}", path.display());
    resources.insert(
        key,
        ResolvedResource {
            logical_name,
            resolved_from,
            path: path.to_string_lossy().to_string(),
        },
    );
}

fn theme_resource_logical_name(theme: &ResolvedTheme, kind: &str, path: &Path) -> String {
    for package in theme.packages.iter().rev() {
        if let Ok(relative) = path.strip_prefix(&package.root) {
            return format!(
                "{kind}:{}@{}:{}",
                package.id,
                package.version,
                relative.to_string_lossy().replace('\\', "/")
            );
        }
    }
    format!(
        "{kind}:{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("resource")
    )
}

fn omnidoc_library_root(config: &MergedConfig) -> PathBuf {
    config
        .lib_path
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            data_local_dir()
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
    Ok(build_input_state(project_path, graph, config, output)?.input_digest)
}

pub fn build_input_state(
    project_path: &Path,
    graph: &DependencyGraph,
    config: &MergedConfig,
    output: &str,
) -> Result<BuildInputState> {
    let mut components = BTreeMap::new();
    components.insert("model:cache-schema".to_string(), CACHE_VERSION.to_string());
    for file in &graph.files {
        let path = project_path.join(file);
        let digest = if path.is_file() {
            content_digest(&path)?
        } else {
            "missing".to_string()
        };
        components.insert(format!("dependency:{file}"), digest);
    }
    let mut resource_occurrences = BTreeMap::<String, usize>::new();
    for resource in &graph.resources {
        let base = format!(
            "resource:{}:{}",
            resource.resolved_from, resource.logical_name
        );
        let occurrence = resource_occurrences.entry(base.clone()).or_default();
        *occurrence += 1;
        components.insert(
            format!("{base}#{}", *occurrence),
            content_digest(Path::new(&resource.path))?,
        );
    }
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
        ("extension_path", format!("{:?}", config.extension_path)),
        ("plugins_enabled", format!("{:?}", config.plugins_enabled)),
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
        (
            "pandoc_pptx_reference_doc",
            format!("{:?}", config.pandoc_pptx_reference_doc),
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
        ("pandoc_toc", format!("{:?}", config.pandoc_toc)),
        (
            "pandoc_embed_resources",
            format!("{:?}", config.pandoc_embed_resources),
        ),
        ("pandoc_lang", format!("{:?}", config.pandoc_lang)),
        ("tectonic_bundle", format!("{:?}", config.tectonic_bundle)),
        (
            "tectonic_only_cached",
            format!("{:?}", config.tectonic_only_cached),
        ),
        (
            "tectonic_shell_escape",
            format!("{:?}", config.tectonic_shell_escape),
        ),
        (
            "tectonic_search_paths",
            format!("{:?}", config.tectonic_search_paths),
        ),
        (
            "tool_paths",
            format!("{:?}", sorted_tool_paths(&config.tool_paths)),
        ),
    ] {
        components.insert(format!("config:{label}"), digest_value(value.as_bytes()));
    }
    for (name, version) in toolchain_versions(project_path, config, output) {
        components.insert(
            format!("toolchain:{name}"),
            digest_value(version.as_bytes()),
        );
    }
    let mut hasher = Hasher::new();
    for (name, value) in &components {
        hash_field(&mut hasher, name, value.as_bytes());
    }
    Ok(BuildInputState {
        input_digest: format_digest(hasher.finalize()),
        components,
    })
}

fn digest_value(value: &[u8]) -> String {
    format_digest(blake3::hash(value))
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
    read_build_cache(project_path, output)
        .is_some_and(|cache| cache.cache_version == CACHE_VERSION && cache.input_digest == digest)
}

pub fn write_cache(project_path: &Path, output: &str, digest: &str) -> Result<()> {
    write_build_cache(
        project_path,
        output,
        &BuildInputState {
            input_digest: digest.to_string(),
            components: BTreeMap::new(),
        },
    )
}

pub fn probe_cache(project_path: &Path, output: &str, state: &BuildInputState) -> CacheProbe {
    let path = cache_path(project_path, output);
    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) => {
            let detail = if error.kind() == std::io::ErrorKind::NotFound {
                "cache_record_missing".to_string()
            } else {
                format!("cache_record_unreadable:{error}")
            };
            return CacheProbe {
                hit: false,
                details: vec![detail],
            };
        }
    };
    let cache = match serde_json::from_str::<BuildCache>(&content) {
        Ok(cache) => cache,
        Err(error) => {
            return CacheProbe {
                hit: false,
                details: vec![format!("cache_record_invalid:{error}")],
            };
        }
    };
    if cache.cache_version != CACHE_VERSION {
        return CacheProbe {
            hit: false,
            details: vec![format!(
                "cache_schema_changed:{}->{}",
                cache.cache_version, CACHE_VERSION
            )],
        };
    }
    if cache.input_digest == state.input_digest {
        return CacheProbe {
            hit: true,
            details: Vec::new(),
        };
    }
    CacheProbe {
        hit: false,
        details: changed_cache_components(&cache.components, &state.components),
    }
}

pub fn write_cache_state(project_path: &Path, output: &str, state: &BuildInputState) -> Result<()> {
    write_build_cache(project_path, output, state)
}

fn read_build_cache(project_path: &Path, output: &str) -> Option<BuildCache> {
    let content = fs::read_to_string(cache_path(project_path, output)).ok()?;
    serde_json::from_str(&content).ok()
}

fn write_build_cache(project_path: &Path, output: &str, state: &BuildInputState) -> Result<()> {
    fs::create_dir_all(project_path.join(CACHE_DIR))?;
    let cache = BuildCache {
        cache_version: CACHE_VERSION,
        input_digest: state.input_digest.clone(),
        components: state.components.clone(),
    };
    let content =
        serde_json::to_string_pretty(&cache).map_err(|err| OmniDocError::Other(err.to_string()))?;
    utils::fs::atomic_write(cache_path(project_path, output), content)?;
    Ok(())
}

fn changed_cache_components(
    previous: &BTreeMap<String, String>,
    current: &BTreeMap<String, String>,
) -> Vec<String> {
    const MAX_DETAILS: usize = 32;
    let keys = previous
        .keys()
        .chain(current.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut details = Vec::new();
    let mut omitted = 0;
    for key in keys {
        let action = match (previous.get(&key), current.get(&key)) {
            (None, Some(_)) => "added",
            (Some(_), None) => "removed",
            (Some(left), Some(right)) if left != right => "changed",
            _ => continue,
        };
        if details.len() >= MAX_DETAILS {
            omitted += 1;
            continue;
        }
        let (kind, name) = key.split_once(':').unwrap_or(("input", key.as_str()));
        details.push(format!("{kind}_{action}:{name}"));
    }
    if omitted > 0 {
        details.push(format!("additional_changes:{omitted}"));
    }
    if details.is_empty() {
        details.push("input_digest_changed_without_component_delta".to_string());
    }
    details
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
    utils::fs::atomic_write(outdir.join(REPORT_FILE), content)?;
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
        packages: combined_locked_packages(project_path, inputs)?,
        toolchain: combined_toolchain_versions(project_path, inputs),
        targets,
    };
    let content =
        toml::to_string_pretty(&lock).map_err(|err| OmniDocError::Other(err.to_string()))?;
    utils::fs::atomic_write(project_path.join(LOCK_FILE), content)?;
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
            packages_up_to_date: false,
            toolchain_up_to_date: false,
            missing_packages: combined_locked_packages(project_path, inputs)?,
            extra_packages: Vec::new(),
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
    let expected_packages = combined_locked_packages(project_path, inputs)?;
    let expected_package_set = expected_packages.iter().cloned().collect::<BTreeSet<_>>();
    let actual_package_set = lock.packages.iter().cloned().collect::<BTreeSet<_>>();
    let missing_packages = expected_package_set
        .difference(&actual_package_set)
        .cloned()
        .collect::<Vec<_>>();
    let extra_packages = actual_package_set
        .difference(&expected_package_set)
        .cloned()
        .collect::<Vec<_>>();
    let packages_up_to_date = missing_packages.is_empty() && extra_packages.is_empty();
    let toolchain_up_to_date = lock.toolchain == combined_toolchain_versions(project_path, inputs);
    let up_to_date = lock.lock_version == LOCK_VERSION
        && missing_targets.is_empty()
        && extra_targets.is_empty()
        && library_up_to_date
        && packages_up_to_date
        && toolchain_up_to_date
        && statuses.values().all(|status| status.up_to_date);

    Ok(LockStatus {
        exists: true,
        up_to_date,
        library_up_to_date,
        packages_up_to_date,
        toolchain_up_to_date,
        missing_packages,
        extra_packages,
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
        revision: library_revision(&library_root),
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
    if !path.join(".git").exists() {
        return None;
    }
    let output = Command::new("git")
        .args(["-C", &path.to_string_lossy(), "rev-parse", "HEAD"])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn library_revision(path: &Path) -> Option<String> {
    git_revision(path).or_else(|| {
        fs::read_to_string(path.join(".omnidoc-release.toml"))
            .ok()
            .and_then(|content| toml::from_str::<toml::Value>(&content).ok())
            .and_then(|metadata| metadata.get("revision")?.as_str().map(str::to_string))
    })
}

fn toolchain_versions(
    project_path: &Path,
    config: &MergedConfig,
    output: &str,
) -> BTreeMap<String, String> {
    let mut versions = [
        ("pandoc", configured_tool(config, "pandoc", "pandoc")),
        (
            "pandoc_crossref",
            configured_tool(config, "pandoc-crossref", "pandoc-crossref"),
        ),
    ]
    .into_iter()
    .map(|(name, program)| (name.to_string(), command_version(&program)))
    .collect::<BTreeMap<_, _>>();
    let output_kind = PandocOutputKind::from_requested(Some(output)).ok();
    if output_kind == Some(PandocOutputKind::Pdf) {
        let executor = BuildExecutor::new(config.tool_paths.clone());
        let preference = latex_engine_preference(project_path, config);
        let engine = executor.resolve_latex_engine(preference).ok();
        if let Some(engine) = engine.as_ref() {
            versions.insert(
                "latex_engine".to_string(),
                command_version(&engine.executable),
            );
            versions.insert(
                "latex_engine_kind".to_string(),
                engine.kind_label().to_string(),
            );
            versions.insert(
                "latex_engine_origin".to_string(),
                engine.origin_label().to_string(),
            );
            if engine.is_tectonic() {
                versions.insert(
                    "tectonic_bundle".to_string(),
                    tectonic_bundle_identity(project_path, config.tectonic_bundle.as_deref()),
                );
            } else {
                versions.insert("tex_kpathsea".to_string(), command_version("kpsewhich"));
            }
        } else {
            versions.insert("latex_engine".to_string(), "unavailable".to_string());
        }
        if let Ok(Some(theme)) = resolve_selected_theme(Some(project_path), config) {
            if !theme.supports_output("pdf") {
                return versions;
            }
            for font in theme.requirements.fonts {
                versions.insert(format!("font:{font}"), font_identity(&font));
            }
            if engine.as_ref().is_some_and(|engine| !engine.is_tectonic()) {
                for package in theme.requirements.system_latex_packages {
                    versions.insert(
                        format!("latex-package:{package}"),
                        latex_package_identity(&package),
                    );
                }
            }
        }
    }
    versions
}

fn tectonic_bundle_identity(project_path: &Path, bundle: Option<&str>) -> String {
    let Some(bundle) = bundle.map(str::trim).filter(|bundle| !bundle.is_empty()) else {
        return "default-web-bundle".to_string();
    };
    if bundle.contains("://") {
        return format!("configured-url:{}", digest_value(bundle.as_bytes()));
    }
    let path = tectonic::resolve_bundle(project_path, bundle);
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("bundle");
    let digest = path
        .is_file()
        .then(|| content_digest(&path).ok())
        .flatten()
        .unwrap_or_else(|| digest_value(bundle.as_bytes()));
    format!("local:{name};digest={digest}")
}

fn latex_engine_preference(project_path: &Path, config: &MergedConfig) -> LatexEnginePreference {
    if detect_project_type(config, project_path) == ProjectType::Latex {
        LatexEnginePreference::Latex
    } else {
        LatexEnginePreference::Markdown
    }
}

fn combined_toolchain_versions(
    project_path: &Path,
    inputs: &[LockTargetInput<'_>],
) -> BTreeMap<String, String> {
    let mut versions = BTreeMap::new();
    for input in inputs {
        versions.extend(toolchain_versions(project_path, input.config, input.output));
    }
    versions
}

fn combined_locked_packages(
    project_path: &Path,
    inputs: &[LockTargetInput<'_>],
) -> Result<Vec<LockedPackage>> {
    let mut packages = BTreeSet::new();
    for input in inputs {
        if let Some(theme) = resolve_selected_theme(Some(project_path), input.config)? {
            for package in theme.packages {
                packages.insert(LockedPackage {
                    kind: package.kind,
                    id: package.id,
                    version: package.version,
                    source: locked_package_source(package.scope).to_string(),
                    digest: package.digest,
                });
            }
        }
        for plugin in enabled_plugins(project_path, input.config)? {
            packages.insert(LockedPackage {
                kind: plugin.package.kind,
                id: plugin.package.id,
                version: plugin.package.version,
                source: locked_package_source(plugin.package.scope).to_string(),
                digest: plugin.package.digest,
            });
        }
    }
    Ok(packages.into_iter().collect())
}

fn locked_package_source(scope: crate::extensions::PackageScope) -> &'static str {
    match scope {
        crate::extensions::PackageScope::Builtin => "builtin",
        crate::extensions::PackageScope::User => "user",
        crate::extensions::PackageScope::Project => "project",
    }
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

fn latex_package_identity(package: &str) -> String {
    if !valid_latex_package_name(package) {
        return "invalid".to_string();
    }
    let file = format!("{package}.sty");
    let output = match Command::new("kpsewhich").args(["--", &file]).output() {
        Ok(output) if output.status.success() => output,
        _ => return "missing".to_string(),
    };
    let path_text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let path = Path::new(&path_text);
    if !path.is_file() {
        return "missing".to_string();
    }
    let version = fs::read_to_string(path)
        .ok()
        .and_then(|content| {
            regex::Regex::new(r"(?m)\\Provides(?:Expl)?Package\{[^}]+\}\s*\[([^]]+)\]")
                .ok()?
                .captures(&content)
                .and_then(|captures| captures.get(1))
                .map(|value| value.as_str().trim().to_string())
        })
        .unwrap_or_else(|| "unknown".to_string());
    let resolved_file = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown");
    let digest = content_digest(path).unwrap_or_else(|_| "unavailable".to_string());
    format!("version={version};file={resolved_file};digest={digest}")
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
    let toolchain = toolchain_versions(context.project_path, context.config, &context.output);
    BuildReport {
        output: context.output,
        target: context.target,
        skipped: context.skipped,
        cache_reason: context.cache_reason,
        cache_details: context.cache_details,
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
    let target = target.trim();
    let target = if let Some(angled) = target.strip_prefix('<') {
        angled.split('>').next().unwrap_or(angled)
    } else {
        target.split_whitespace().next().unwrap_or(target)
    };
    if !is_local_path(target) {
        return;
    }
    let target = target
        .split(['#', '?'])
        .next()
        .map(|value| percent_decode_str(value).decode_utf8_lossy())
        .unwrap_or_default();
    if target.is_empty() {
        return;
    }
    let base = source_file.parent().unwrap_or(project_path);
    if !base.join(target.as_ref()).exists() && !project_path.join(target.as_ref()).exists() {
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
        .or_else(|| data_local_dir().map(|path| path.join("omnidoc")));
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

fn is_local_path(target: &str) -> bool {
    !target.starts_with("http://")
        && !target.starts_with("https://")
        && !target.starts_with("mailto:")
        && !target.starts_with('#')
        && !looks_like_email_address(target)
}

fn looks_like_email_address(target: &str) -> bool {
    let target = target.trim_matches(['<', '>']);
    let Some((local, domain)) = target.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && !domain.is_empty()
        && !target.contains(['/', '\\', ' '])
        && domain.contains('.')
}

fn cache_path(project_path: &Path, output: &str) -> PathBuf {
    project_path
        .join(CACHE_DIR)
        .join(format!("build-{}.json", output))
}

fn display_relative(project_path: &Path, path: &Path) -> String {
    let relative = path
        .strip_prefix(project_path)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();
    relative.replace('\\', "/")
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
        acquire_project_write_lock, build_input_digest, build_report, cache_hit,
        changed_cache_components, check_lock, dependency_graph, filter_depfile_metadata_key,
        filter_depfile_name, latex_engine_preference, lint_project, pandoc_option_file_references,
        supported_outputs, validate_config, write_cache, write_lock, write_lock_targets, LockFile,
        LockTargetInput, CACHE_DIR, INCLUDE_DEPFILE, LATEX_INPUT_DEPFILE,
    };
    use crate::build::executor::LatexEnginePreference;
    use crate::config::MergedConfig;
    use std::collections::BTreeMap;
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

    fn canonical_text(path: &Path) -> String {
        path.canonicalize()
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .to_string()
    }

    #[test]
    fn project_write_lock_rejects_concurrent_writers_and_recovers() {
        let project = tempfile::tempdir().expect("project");
        let first = acquire_project_write_lock(project.path(), "first build").expect("first lock");

        let error = acquire_project_write_lock(project.path(), "second build")
            .err()
            .expect("second writer should fail");
        assert!(error.to_string().contains("another OmniDoc process"));

        drop(first);
        acquire_project_write_lock(project.path(), "retry build").expect("released lock");
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
    fn inferred_main_tex_uses_the_native_latex_engine_policy() {
        let project = tempfile::tempdir().expect("project");
        fs::write(
            project.path().join("main.tex"),
            "\\documentclass{article}\n",
        )
        .expect("entry");

        assert_eq!(
            latex_engine_preference(project.path(), &MergedConfig::default()),
            LatexEnginePreference::Latex
        );
    }

    #[test]
    fn external_relative_tectonic_bundle_is_a_tracked_resource() {
        let root = tempfile::tempdir().expect("root");
        let project = root.path().join("project");
        fs::create_dir_all(&project).expect("project directory");
        fs::write(project.join("main.md"), "# Book\n").expect("entry");
        let bundle = root.path().join("bundle.tar");
        fs::write(&bundle, "bundle-v1").expect("bundle");
        let config = MergedConfig {
            entry: Some("main.md".to_string()),
            tectonic_bundle: Some("../bundle.tar".to_string()),
            ..Default::default()
        };

        let graph = dependency_graph(&project, &config);

        assert!(graph.resources.iter().any(|resource| {
            resource.logical_name == "tectonic-bundle"
                && Path::new(&resource.path) == bundle.canonicalize().expect("canonical bundle")
        }));
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
    fn extracts_file_references_from_pandoc_options() {
        let references = pandoc_option_file_references(&[
            "--toc".to_string(),
            "--include-in-header=header.tex".to_string(),
            "--bibliography".to_string(),
            "references.bib".to_string(),
            "-H".to_string(),
            "short-header.tex".to_string(),
        ]);
        assert_eq!(
            references,
            [
                ("--include-in-header".to_string(), "header.tex".to_string()),
                ("--bibliography".to_string(), "references.bib".to_string()),
                ("-H".to_string(), "short-header.tex".to_string()),
            ]
        );
    }

    #[test]
    fn explains_changed_cache_components() {
        let previous = BTreeMap::from([
            ("dependency:main.md".to_string(), "v1".to_string()),
            ("toolchain:pandoc".to_string(), "3.9".to_string()),
        ]);
        let current = BTreeMap::from([
            ("dependency:main.md".to_string(), "v2".to_string()),
            (
                "resource:external:latex-fls-input:fontspec.sty#1".to_string(),
                "digest".to_string(),
            ),
        ]);
        let details = changed_cache_components(&previous, &current);
        assert!(details.contains(&"dependency_changed:main.md".to_string()));
        assert!(
            details.contains(&"resource_added:external:latex-fls-input:fontspec.sty#1".to_string())
        );
        assert!(details.contains(&"toolchain_removed:pandoc".to_string()));
    }

    #[test]
    fn pdf_dependency_graph_hashes_fls_inputs() {
        let project = tempfile::tempdir().expect("project tempdir");
        let external = tempfile::tempdir().expect("external texmf");
        let chapter = project.path().join("chapter.tex");
        let package = external.path().join("indirect-theme.sty");
        fs::write(project.path().join("main.md"), "# Main\n").expect("entry");
        fs::write(&chapter, "Project LaTeX input\n").expect("project TeX input");
        fs::write(&package, "External package v1\n").expect("external package");
        fs::create_dir_all(project.path().join(CACHE_DIR)).expect("cache dir");
        fs::write(
            project.path().join(CACHE_DIR).join(LATEX_INPUT_DEPFILE),
            format!(
                "# omnidoc-depfile-v1\n# source=latex-fls\n{}\n{}\n",
                chapter.display(),
                package.display()
            ),
        )
        .expect("LaTeX depfile");
        let config = MergedConfig {
            entry: Some("main.md".to_string()),
            to: Some("pdf".to_string()),
            ..Default::default()
        };

        let graph = dependency_graph(project.path(), &config);
        assert!(graph.files.contains(&"chapter.tex".to_string()));
        assert!(graph.resources.iter().any(|resource| {
            resource.logical_name.ends_with("indirect-theme.sty")
                && resource.resolved_from == "external"
                && resource.path == canonical_text(&package)
        }));
        let before =
            build_input_digest(project.path(), &graph, &config, "pdf").expect("initial PDF digest");
        fs::write(&package, "External package v2\n").expect("updated external package");
        let after =
            build_input_digest(project.path(), &graph, &config, "pdf").expect("updated PDF digest");
        assert_ne!(before, after);
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
            resource.logical_name == format!("filter-depfile:{}", INCLUDE_DEPFILE)
                && resource.resolved_from == "external"
                && resource.path == canonical_text(external.path())
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
    fn dependency_graph_consumes_depfiles_for_custom_active_filters() {
        let project = tempfile::tempdir().expect("project tempdir");
        fs::write(
            project.path().join(".omnidoc.toml"),
            "[project]\nentry='main.md'\n",
        )
        .expect("config");
        fs::write(project.path().join("main.md"), "# Main\n").expect("entry");
        fs::create_dir_all(project.path().join("data")).expect("data dir");
        fs::write(project.path().join("data/custom.json"), "{}\n").expect("custom data");
        fs::create_dir_all(project.path().join(CACHE_DIR)).expect("cache dir");
        fs::write(
            project.path().join(CACHE_DIR).join("custom-reader.d"),
            format!(
                "# omnidoc-depfile-v1\n{}\n",
                project.path().join("data/custom.json").display()
            ),
        )
        .expect("custom depfile");
        fs::write(
            project.path().join(CACHE_DIR).join(INCLUDE_DEPFILE),
            format!(
                "# omnidoc-depfile-v1\n{}\n",
                project.path().join("inactive.md").display()
            ),
        )
        .expect("inactive depfile");
        fs::write(project.path().join("inactive.md"), "# Inactive\n").expect("inactive source");

        let graph = dependency_graph(
            project.path(),
            &MergedConfig {
                entry: Some("main.md".to_string()),
                to: Some("html".to_string()),
                pandoc_lua_filters: vec!["filters/Custom Reader.lua".to_string()],
                ..Default::default()
            },
        );

        assert!(graph.files.contains(&"data/custom.json".to_string()));
        assert!(!graph.files.contains(&"inactive.md".to_string()));
        assert_eq!(
            filter_depfile_name("filters/Custom Reader.lua").as_deref(),
            Some("custom-reader.d")
        );
        assert_eq!(
            filter_depfile_metadata_key("filters/Custom Reader.lua").as_deref(),
            Some("omnidoc-depfile-custom-reader")
        );
    }

    #[test]
    fn lint_resolves_percent_encoded_and_angle_wrapped_resource_paths() {
        let project = tempfile::tempdir().expect("project tempdir");
        fs::create_dir_all(project.path().join("assets")).expect("assets dir");
        fs::write(project.path().join("assets/team map.png"), b"png").expect("image");
        fs::write(
            project.path().join("main.md"),
            "![Map](assets/team%20map.png)\n\n[Map](<assets/team%20map.png>)\n\n[Site](<https://example.com/a>)\n",
        )
        .expect("entry");

        let issues = lint_project(project.path());

        assert!(
            issues.is_empty(),
            "encoded local paths should resolve: {issues:#?}"
        );
    }

    #[test]
    fn lint_uses_source_specific_resource_syntax_and_ignores_code_examples() {
        let project = tempfile::tempdir().expect("project tempdir");
        fs::create_dir_all(project.path().join("figures")).expect("figures dir");
        fs::create_dir_all(project.path().join("tex")).expect("TeX source dir");
        fs::write(project.path().join("figures/soc.svg"), "<svg/>\n").expect("figure");
        fs::write(project.path().join("tex/chapter.tex"), "Chapter\n").expect("chapter");
        fs::write(project.path().join("references.bib"), "@book{x}\n").expect("bibliography");
        fs::write(
            project.path().join("main.tex"),
            r#"\input{chapter}
\includegraphics[width=1cm]{figures/soc}
\bibliography{references}
\draw (4,0) -- (5,1);
std::vector<int> values;
user@example.com
\begin{verbatim}
![example](missing-in-code.png)
\end{verbatim}
% \includegraphics{missing-in-comment.png}
"#,
        )
        .expect("LaTeX entry");
        fs::write(
            project.path().join("notes.md"),
            "[Contact](user@example.com)\n\n```markdown\n![example](missing-in-fence.png)\n```\n\n    [code](missing-indented.txt)\n",
        )
        .expect("Markdown notes");

        let issues = lint_project(project.path());

        assert!(
            issues.is_empty(),
            "code and non-resource syntax should be ignored: {issues:#?}"
        );
    }

    #[test]
    fn lint_reports_missing_markdown_and_latex_resources() {
        let project = tempfile::tempdir().expect("project tempdir");
        fs::write(
            project.path().join("main.md"),
            "![Missing](images/missing.png)\n",
        )
        .expect("Markdown entry");
        fs::write(
            project.path().join("main.tex"),
            "\\input{chapters/missing}\n\\includegraphics{figures/missing}\n",
        )
        .expect("LaTeX entry");

        let issues = lint_project(project.path());
        let messages = issues
            .iter()
            .map(|issue| issue.message.as_str())
            .collect::<Vec<_>>();

        assert_eq!(issues.len(), 3, "unexpected lint issues: {issues:#?}");
        assert!(messages
            .iter()
            .any(|message| message.contains("images/missing.png")));
        assert!(messages
            .iter()
            .any(|message| message.contains("chapters/missing")));
        assert!(messages
            .iter()
            .any(|message| message.contains("figures/missing")));
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
        fs::write(
            library.path().join(".omnidoc-release.toml"),
            "contract_version = 1\nversion = '1.0.0'\nrevision = 'v1.0.0'\narchive_url = 'https://example.invalid/libs.tar.gz'\narchive_digest = 'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'\n",
        )
        .expect("release metadata");
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
                && resource.path == canonical_text(&css)
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
        assert!(graph
            .resources
            .iter()
            .any(|resource| resource.logical_name == "omnidoc-libs-release"));
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
        let lock: LockFile = toml::from_str(&lock_text).expect("lock v5");
        assert_eq!(lock.lock_version, 5);
        let locked_library = lock.library.as_ref().expect("locked library");
        assert_eq!(locked_library.version.as_deref(), Some("1.0.0"));
        assert_eq!(locked_library.revision.as_deref(), Some("v1.0.0"));
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
            project_path: project.path(),
            output: "html".to_string(),
            target: "book".to_string(),
            skipped: true,
            cache_reason: "input_digest_match".to_string(),
            cache_details: Vec::new(),
            duration_ms: 12,
            input_digest: "blake3:input".to_string(),
            graph: &graph,
            config: &config,
            artifact: &artifact,
            compatibility: None,
            issues: Vec::new(),
        });

        assert_eq!(report.cache_reason, "input_digest_match");
        assert!(report.cache_details.is_empty());
        assert_eq!(report.duration_ms, 12);
        assert!(report
            .artifact_digest
            .as_deref()
            .is_some_and(|digest| digest.starts_with("blake3:")));
        assert!(report.toolchain.contains_key("pandoc"));
        assert!(!report.toolchain.contains_key("latex_engine"));
    }

    #[test]
    fn lock_v5_keeps_multiple_output_targets() {
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
        let lock: LockFile = toml::from_str(&lock_text).expect("lock v5");
        assert_eq!(lock.lock_version, 5);
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
    fn active_theme_packages_invalidate_cache_and_lock_identity() {
        let project = temporary_project("theme-package-project");
        let store = temporary_project("theme-package-store");
        let package = store.join("themes/acme/cache-theme/1.0.0");
        fs::create_dir_all(package.join("styles")).expect("theme package");
        fs::create_dir_all(&project).expect("project");
        fs::write(project.join("main.md"), "# Guide\n").expect("entry");
        fs::write(package.join("styles/theme.css"), "body { color: #111; }\n").expect("theme CSS");
        fs::write(
            package.join("omnidoc-package.toml"),
            r##"manifest_version = 2
kind = "theme"
id = "acme/cache-theme"
version = "1.0.0"
compatible_omnidoc = ">=1.8,<2"

[theme]
api_version = 1
outputs = ["html"]

[theme.resources]
html_css = ["styles/theme.css"]

[theme.tokens.color]
text = "#111111"
"##,
        )
        .expect("theme manifest");
        let config = MergedConfig {
            entry: Some("main.md".to_string()),
            to: Some("html".to_string()),
            theme_name: Some("acme/cache-theme".to_string()),
            theme_version: Some("=1.0.0".to_string()),
            extension_path: Some(store.to_string_lossy().to_string()),
            project_root: Some(project.to_string_lossy().to_string()),
            ..Default::default()
        };

        let first_graph = dependency_graph(&project, &config);
        let first_digest =
            build_input_digest(&project, &first_graph, &config, "html").expect("first digest");
        write_lock(&project, &config, &first_graph).expect("write lock");
        let lock: LockFile = toml::from_str(
            &fs::read_to_string(project.join("omnidoc.lock")).expect("lock content"),
        )
        .expect("lock file");
        assert_eq!(lock.packages.len(), 1);
        assert_eq!(lock.packages[0].id, "acme/cache-theme");

        fs::write(package.join("styles/theme.css"), "body { color: #222; }\n")
            .expect("updated theme CSS");
        let second_graph = dependency_graph(&project, &config);
        let second_digest =
            build_input_digest(&project, &second_graph, &config, "html").expect("second digest");
        assert_ne!(first_digest, second_digest);
        let status = check_lock(&project, &config, &second_graph).expect("lock status");
        assert!(!status.packages_up_to_date);
        assert_eq!(status.missing_packages.len(), 1);
        assert_eq!(status.extra_packages.len(), 1);

        fs::remove_dir_all(project).expect("project cleanup");
        fs::remove_dir_all(store).expect("store cleanup");
    }
}
