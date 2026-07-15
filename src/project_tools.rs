use crate::config::MergedConfig;
use crate::constants::pandoc;
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
const LOCK_FILE: &str = "omnidoc.lock";
const REPORT_FILE: &str = "omnidoc-report.json";
const LOCK_VERSION: u32 = 2;

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
    pub input_digest: String,
    pub dependencies: Vec<String>,
    pub issues: Vec<ProjectIssue>,
    pub timestamp_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildReportDocument {
    pub omnidoc_version: String,
    pub generated_at_unix: u64,
    pub reports: Vec<BuildReport>,
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
    pub input_digest: String,
    pub library: Option<LockedLibrary>,
    pub toolchain: BTreeMap<String, String>,
    pub resources: Vec<LockedResource>,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LockedLibrary {
    pub revision: Option<String>,
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
        }
    }

    DependencyGraph {
        files: files.into_iter().collect(),
        resources: resolved_build_resources(project_path, config),
    }
}

fn resolved_build_resources(project_path: &Path, config: &MergedConfig) -> Vec<ResolvedResource> {
    let library_root = omnidoc_library_root(config);
    let mut resources = BTreeMap::<String, ResolvedResource>::new();

    let filters = if config.pandoc_lua_filters.is_empty() {
        vec![
            "include-files.lua",
            "include-code-files.lua",
            "diagram-generator.lua",
            "admonition.lua",
            "ltblr.lua",
            "latex-patch.lua",
            "fonts-and-alignment.lua",
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>()
    } else {
        config.pandoc_lua_filters.clone()
    };
    for filter in filters {
        // Keep this resolution identical to PandocBuilder::push_lua_filters:
        // filter names are relative to the shared filter directory.
        if let Some(path) =
            existing_path(library_root.join(pandoc::LIB_PANDOC_FILTERS).join(&filter))
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

    let configured_css = config.pandoc_css.as_deref();
    for (logical_name, configured, fallback) in [
        ("html-css", configured_css, pandoc::LIB_PANDOC_CSS_DEFAULT),
        (
            "epub-css",
            config.pandoc_epub_css.as_deref().or(configured_css),
            "pandoc/data/epub.css",
        ),
    ] {
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

    for (logical_name, configured, fallback) in [
        (
            "latex-template",
            config
                .pandoc_latex_template
                .as_deref()
                .or(config.pandoc_template.as_deref()),
            Some(pandoc::DEFAULT_TEMPLATE_LATEX),
        ),
        (
            "html-template",
            config.pandoc_html_template.as_deref(),
            None,
        ),
        (
            "epub-template",
            config.pandoc_epub_template.as_deref(),
            None,
        ),
    ] {
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

    if let Some(reference_doc) = config.pandoc_reference_doc.as_deref() {
        if let Some(path) = resolve_resource_path(project_path, &library_root, reference_doc, None)
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

    for (logical_name, path) in [
        (
            "crossref-yaml",
            config
                .pandoc_crossref_yaml
                .as_deref()
                .and_then(|value| resolve_resource_path(project_path, &library_root, value, None))
                .or_else(|| existing_path(library_root.join(pandoc::LIB_PANDOC_CROSSREF_YAML))),
        ),
        ("texmf", existing_path(library_root.join("texmf"))),
    ] {
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
        ("pandoc_options", format!("{:?}", config.pandoc_options)),
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

fn content_digest(path: &Path) -> Result<String> {
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
        .map(|cache| cache.cache_version == LOCK_VERSION && cache.input_digest == digest)
        .unwrap_or(false)
}

pub fn write_cache(project_path: &Path, output: &str, digest: &str) -> Result<()> {
    fs::create_dir_all(project_path.join(CACHE_DIR))?;
    let cache = BuildCache {
        cache_version: LOCK_VERSION,
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
    let digest = input_digest(project_path, graph)?;
    let resources = locked_resources(graph)?;
    let lock = LockFile {
        lock_version: LOCK_VERSION,
        omnidoc_version: env!("CARGO_PKG_VERSION").to_string(),
        input_digest: digest,
        library: locked_library(config, &resources),
        toolchain: toolchain_versions(config),
        resources,
        dependencies: graph.files.clone(),
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
    let expected_digest = input_digest(project_path, graph)?;
    let lock_path = project_path.join(LOCK_FILE);
    if !lock_path.exists() {
        return Ok(LockStatus {
            exists: false,
            up_to_date: false,
            expected_digest,
            actual_digest: None,
            missing_dependencies: graph.files.clone(),
            extra_dependencies: Vec::new(),
        });
    }

    let content = fs::read_to_string(&lock_path)?;
    let lock: LockFile =
        toml::from_str(&content).map_err(|err| OmniDocError::Other(err.to_string()))?;
    let expected_dependencies = graph.files.iter().cloned().collect::<BTreeSet<_>>();
    let actual_dependencies = lock.dependencies.iter().cloned().collect::<BTreeSet<_>>();
    let missing_dependencies = expected_dependencies
        .difference(&actual_dependencies)
        .cloned()
        .collect::<Vec<_>>();
    let extra_dependencies = actual_dependencies
        .difference(&expected_dependencies)
        .cloned()
        .collect::<Vec<_>>();
    let expected_resources = locked_resources(graph)?;
    let up_to_date = lock.lock_version == LOCK_VERSION
        && lock.input_digest == expected_digest
        && lock.resources == expected_resources
        && lock.library == locked_library(config, &expected_resources)
        && lock.toolchain == toolchain_versions(config)
        && missing_dependencies.is_empty()
        && extra_dependencies.is_empty();

    Ok(LockStatus {
        exists: true,
        up_to_date,
        expected_digest,
        actual_digest: Some(lock.input_digest),
        missing_dependencies,
        extra_dependencies,
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
    Some(LockedLibrary {
        revision: git_revision(&omnidoc_library_root(config)),
        digest: format_digest(hasher.finalize()),
    })
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

fn toolchain_versions(config: &MergedConfig) -> BTreeMap<String, String> {
    let latex_engine = config
        .tool_paths
        .get("latex_engine")
        .and_then(|value| value.clone())
        .unwrap_or_else(|| "xelatex".to_string());
    [
        ("pandoc", configured_tool(config, "pandoc", "pandoc")),
        (
            "pandoc_crossref",
            configured_tool(config, "pandoc_crossref", "pandoc-crossref"),
        ),
        ("latex_engine", latex_engine),
    ]
    .into_iter()
    .map(|(name, program)| (name.to_string(), command_version(&program)))
    .collect()
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

pub fn build_report(
    output: String,
    target: String,
    skipped: bool,
    input_digest: String,
    dependencies: Vec<String>,
    issues: Vec<ProjectIssue>,
) -> BuildReport {
    BuildReport {
        output,
        target,
        skipped,
        input_digest,
        dependencies,
        issues,
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
        build_input_digest, cache_hit, dependency_graph, hook_argv, manifest_hook_names,
        parse_lint_rule_output, supported_outputs, validate_config, validate_hook_command,
        write_cache, write_lock, HookCommand, LoadedPlugin, LockFile, PluginHooks, PluginInfo,
        PluginManifest,
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
    fn shared_resources_invalidate_cache_and_lock_uses_portable_digests() {
        let project = tempfile::tempdir().expect("project tempdir");
        let library = tempfile::tempdir().expect("library tempdir");
        let css_dir = library.path().join("pandoc/css");
        fs::create_dir_all(&css_dir).expect("css dir");
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
        let lock: LockFile = toml::from_str(&lock_text).expect("lock v2");
        assert_eq!(lock.lock_version, 2);
        assert!(lock.input_digest.starts_with("blake3:"));
        assert!(lock.resources.iter().any(|resource| {
            resource.logical_name == "html-css"
                && resource.resolved_from == "omnidoc-libs"
                && resource.digest.starts_with("blake3:")
        }));
        assert!(!lock_text.contains(&library.path().to_string_lossy().to_string()));
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
