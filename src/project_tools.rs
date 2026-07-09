use crate::config::MergedConfig;
use crate::error::{OmniDocError, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;

const CACHE_DIR: &str = ".omnidoc-cache";
const LOCK_FILE: &str = "omnidoc.lock";
const REPORT_FILE: &str = "omnidoc-report.json";

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildReport {
    pub output: String,
    pub target: String,
    pub skipped: bool,
    pub input_hash: u64,
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
    input_hash: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockFile {
    pub omnidoc_version: String,
    pub lib_path: Option<String>,
    pub lib_url: Option<String>,
    pub input_hash: u64,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockStatus {
    pub exists: bool,
    pub up_to_date: bool,
    pub expected_hash: u64,
    pub actual_hash: Option<u64>,
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
        check_configured_path(
            project_path,
            css,
            "Configured pandoc.css not found",
            false,
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
        check_configured_path(
            project_path,
            epub_css,
            "Configured pandoc.epub_css not found",
            false,
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
    if project_path.join(".omnidoc.toml").exists() {
        files.insert(".omnidoc.toml".to_string());
    }
    if let Some(entry) = &config.entry {
        if project_path.join(entry).exists() {
            files.insert(entry.clone());
        }
    }

    for file in source_files(project_path) {
        files.insert(display_relative(project_path, &file));
    }

    DependencyGraph {
        files: files.into_iter().collect(),
    }
}

pub fn input_hash(project_path: &Path, graph: &DependencyGraph) -> Result<u64> {
    let mut hasher = DefaultHasher::new();
    hash_dependency_files(project_path, graph, &mut hasher)?;
    Ok(hasher.finish())
}

pub fn build_input_hash(
    project_path: &Path,
    graph: &DependencyGraph,
    config: &MergedConfig,
    output: &str,
) -> Result<u64> {
    let mut hasher = DefaultHasher::new();
    hash_dependency_files(project_path, graph, &mut hasher)?;
    output.hash(&mut hasher);
    config.from.hash(&mut hasher);
    config.to.hash(&mut hasher);
    config.target.hash(&mut hasher);
    config.outdir.hash(&mut hasher);
    config.author.hash(&mut hasher);
    config.metadata_file.hash(&mut hasher);
    config.latex_backend.hash(&mut hasher);
    config.max_latex_passes.hash(&mut hasher);
    config.figure_paths.hash(&mut hasher);
    config.figure_output.hash(&mut hasher);
    config.pandoc_options.hash(&mut hasher);
    config.pandoc_css.hash(&mut hasher);
    config.pandoc_reference_doc.hash(&mut hasher);
    config.pandoc_epub_css.hash(&mut hasher);
    config.pandoc_from_format.hash(&mut hasher);
    config.pandoc_to_format.hash(&mut hasher);
    config.pandoc_lua_filters.hash(&mut hasher);
    config.pandoc_template.hash(&mut hasher);
    config.pandoc_html_template.hash(&mut hasher);
    config.pandoc_latex_template.hash(&mut hasher);
    config.pandoc_epub_template.hash(&mut hasher);
    config.pandoc_data_dir.hash(&mut hasher);
    config.pandoc_resource_path.hash(&mut hasher);
    config.pandoc_syntax_highlighting.hash(&mut hasher);
    config.pandoc_crossref_yaml.hash(&mut hasher);
    config.pandoc_python_path.hash(&mut hasher);
    config.pandoc_standalone.hash(&mut hasher);
    config.pandoc_embed_resources.hash(&mut hasher);
    config.pandoc_lang.hash(&mut hasher);
    sorted_tool_paths(&config.tool_paths).hash(&mut hasher);
    Ok(hasher.finish())
}

fn hash_dependency_files(
    project_path: &Path,
    graph: &DependencyGraph,
    hasher: &mut DefaultHasher,
) -> Result<()> {
    for file in &graph.files {
        file.hash(hasher);
        let path = project_path.join(file);
        if path.is_file() {
            fs::read(&path)?.hash(hasher);
        }
    }
    Ok(())
}

fn sorted_tool_paths(
    tool_paths: &std::collections::HashMap<String, Option<String>>,
) -> BTreeMap<String, Option<String>> {
    tool_paths
        .iter()
        .map(|(tool, path)| (tool.clone(), path.clone()))
        .collect()
}

pub fn cache_hit(project_path: &Path, output: &str, hash: u64) -> bool {
    let path = cache_path(project_path, output);
    let Ok(content) = fs::read_to_string(path) else {
        return false;
    };
    serde_json::from_str::<BuildCache>(&content)
        .map(|cache| cache.input_hash == hash)
        .unwrap_or(false)
}

pub fn write_cache(project_path: &Path, output: &str, hash: u64) -> Result<()> {
    fs::create_dir_all(project_path.join(CACHE_DIR))?;
    let cache = BuildCache { input_hash: hash };
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
    let hash = input_hash(project_path, graph)?;
    let lock = LockFile {
        omnidoc_version: env!("CARGO_PKG_VERSION").to_string(),
        lib_path: config.lib_path.clone(),
        lib_url: config.lib_url.clone(),
        input_hash: hash,
        dependencies: graph.files.clone(),
    };
    let content =
        toml::to_string_pretty(&lock).map_err(|err| OmniDocError::Other(err.to_string()))?;
    fs::write(project_path.join(LOCK_FILE), content)?;
    Ok(())
}

pub fn check_lock(
    project_path: &Path,
    _config: &MergedConfig,
    graph: &DependencyGraph,
) -> Result<LockStatus> {
    let expected_hash = input_hash(project_path, graph)?;
    let lock_path = project_path.join(LOCK_FILE);
    if !lock_path.exists() {
        return Ok(LockStatus {
            exists: false,
            up_to_date: false,
            expected_hash,
            actual_hash: None,
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
    let up_to_date = lock.input_hash == expected_hash
        && missing_dependencies.is_empty()
        && extra_dependencies.is_empty();

    Ok(LockStatus {
        exists: true,
        up_to_date,
        expected_hash,
        actual_hash: Some(lock.input_hash),
        missing_dependencies,
        extra_dependencies,
    })
}

pub fn discovered_plugins(project_path: &Path, config: &MergedConfig) -> Vec<PluginInfo> {
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
            plugins.push(read_plugin_manifest(&manifest_path));
        }
    }
    plugins.sort_by(|left, right| left.path.cmp(&right.path));
    plugins.dedup_by(|left, right| left.path == right.path);
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
    input_hash: u64,
    dependencies: Vec<String>,
    issues: Vec<ProjectIssue>,
) -> BuildReport {
    BuildReport {
        output,
        target,
        skipped,
        input_hash,
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

fn read_plugin_manifest(path: &Path) -> PluginInfo {
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
            return invalid_plugin(
                path,
                fallback_key,
                format!("Failed to read manifest: {}", err),
            );
        }
    };
    let manifest = match toml::from_str::<PluginManifest>(&content) {
        Ok(manifest) => manifest,
        Err(err) => {
            return invalid_plugin(
                path,
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
    PluginInfo {
        path: path.display().to_string(),
        key,
        name: manifest.name,
        version: manifest.version,
        description: manifest.description,
        kind,
        valid: error.is_none(),
        error,
    }
}

fn invalid_plugin(path: &Path, key: String, error: String) -> PluginInfo {
    PluginInfo {
        path: path.display().to_string(),
        key,
        name: None,
        version: None,
        description: None,
        kind: "plugin".to_string(),
        valid: false,
        error: Some(error),
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

    None
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
    use super::{supported_outputs, validate_config};
    use crate::config::MergedConfig;
    use std::path::Path;

    #[test]
    fn validates_unsupported_output() {
        let mut config = MergedConfig::default();
        config.to = Some("unknown".to_string());
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
    fn validates_unsupported_build_outputs() {
        let mut config = MergedConfig::default();
        config.outputs = vec!["pdf".to_string(), "unknown".to_string()];
        let issues = validate_config(Path::new("."), &config);

        assert!(issues
            .iter()
            .any(|issue| issue.message.contains("build.outputs")));
    }

    #[test]
    fn validates_engine_pass_count() {
        let mut config = MergedConfig::default();
        config.latex_backend = "engine".to_string();
        config.max_latex_passes = 0;
        let issues = validate_config(Path::new("."), &config);

        assert!(issues
            .iter()
            .any(|issue| issue.message.contains("max_latex_passes")));
    }
}
