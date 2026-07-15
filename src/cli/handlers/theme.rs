use crate::cli::commands::ThemeSubcommand;
use crate::cli::handlers::lib::configured_library_path;
use crate::config::global::GlobalConfig;
use crate::error::{OmniDocError, Result};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

const THEME_MANIFEST_VERSION: u32 = 1;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct ThemeManifest {
    pub(crate) manifest_version: u32,
    pub(crate) name: String,
    pub(crate) version: String,
    #[serde(default)]
    pub(crate) description: Option<String>,
    pub(crate) compatible_omnidoc: String,
    #[serde(default)]
    pub(crate) compatibility: Option<String>,
    #[serde(default)]
    pub(crate) resources: ThemeResources,
    #[serde(default)]
    pub(crate) requirements: ThemeRequirements,
    #[serde(default)]
    pub(crate) metadata: ThemeMetadata,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub(crate) struct ThemeResources {
    #[serde(default)]
    pub(crate) html_css: Vec<String>,
    #[serde(default)]
    pub(crate) epub_css: Vec<String>,
    #[serde(default)]
    pub(crate) latex_packages: Vec<String>,
    #[serde(default)]
    pub(crate) latex_headers: Vec<String>,
    #[serde(default)]
    pub(crate) lua_filters: Vec<String>,
    #[serde(default)]
    pub(crate) templates: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub(crate) struct ThemeRequirements {
    #[serde(default)]
    pub(crate) fonts: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub(crate) struct ThemeMetadata {
    #[serde(default)]
    pub(crate) defaults: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
struct ThemeReport {
    manifest: String,
    #[serde(flatten)]
    theme: Option<ThemeManifest>,
    compatible: Option<bool>,
    valid: bool,
    errors: Vec<String>,
}

pub fn handle_theme(subcommand: ThemeSubcommand) -> Result<()> {
    let global_config = GlobalConfig::load()?;
    let library = configured_library_path(&global_config)?;
    match subcommand {
        ThemeSubcommand::List { json } => {
            let reports = load_theme_reports(&library)?;
            print_reports(&reports, json, false)?;
        }
        ThemeSubcommand::Inspect { name, json } => {
            let report = load_named_theme(&library, &name)?;
            print_reports(std::slice::from_ref(&report), json, true)?;
        }
        ThemeSubcommand::Validate { name, json } => {
            let reports = match name {
                Some(name) => vec![load_named_theme(&library, &name)?],
                None => load_theme_reports(&library)?,
            };
            print_reports(&reports, json, false)?;
            ensure_valid(&reports)?;
        }
    }
    Ok(())
}

fn load_named_theme(library: &Path, name: &str) -> Result<ThemeReport> {
    if safe_relative_path(name).is_none() || name.contains('/') || name.contains('\\') {
        return Err(OmniDocError::Other(format!("invalid theme name: {}", name)));
    }
    let path = library.join("themes").join(format!("{}.toml", name));
    if !path.is_file() {
        return Err(OmniDocError::Other(format!(
            "theme '{}' is not installed in {}",
            name,
            library.join("themes").display()
        )));
    }
    Ok(inspect_manifest(library, &path))
}

pub(crate) fn load_theme_manifest(library: &Path, name: &str) -> Result<ThemeManifest> {
    let report = load_named_theme(library, name)?;
    if !report.valid {
        return Err(OmniDocError::Other(format!(
            "theme '{}' is invalid: {}",
            name,
            report.errors.join("; ")
        )));
    }
    report.theme.ok_or_else(|| {
        OmniDocError::Other(format!("theme '{}' manifest could not be loaded", name))
    })
}

fn load_theme_reports(library: &Path) -> Result<Vec<ThemeReport>> {
    let directory = library.join("themes");
    if !directory.is_dir() {
        return Ok(Vec::new());
    }
    let mut paths = fs::read_dir(&directory)
        .map_err(OmniDocError::Io)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("toml"))
        .collect::<Vec<_>>();
    paths.sort();
    Ok(paths
        .iter()
        .map(|path| inspect_manifest(library, path))
        .collect())
}

fn inspect_manifest(library: &Path, path: &Path) -> ThemeReport {
    let relative_manifest = path
        .strip_prefix(library)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    let mut report = ThemeReport {
        manifest: relative_manifest,
        theme: None,
        compatible: None,
        valid: false,
        errors: Vec::new(),
    };
    if path
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        report
            .errors
            .push("theme manifest must not be a symbolic link".to_string());
        return report;
    }
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) => {
            report
                .errors
                .push(format!("cannot read manifest: {}", error));
            return report;
        }
    };
    let manifest = match toml::from_str::<ThemeManifest>(&content) {
        Ok(manifest) => manifest,
        Err(error) => {
            report
                .errors
                .push(format!("invalid theme manifest: {}", error));
            return report;
        }
    };
    validate_manifest(library, path, &manifest, &mut report);
    report.valid = report.errors.is_empty();
    report.theme = Some(manifest);
    report
}

fn validate_manifest(
    library: &Path,
    path: &Path,
    manifest: &ThemeManifest,
    report: &mut ThemeReport,
) {
    if manifest.manifest_version != THEME_MANIFEST_VERSION {
        report.errors.push(format!(
            "unsupported manifest_version {}",
            manifest.manifest_version
        ));
    }
    let file_name = path.file_stem().and_then(|name| name.to_str());
    if file_name != Some(manifest.name.as_str()) {
        report.errors.push(format!(
            "theme name '{}' does not match manifest filename",
            manifest.name
        ));
    }
    if manifest.name.trim().is_empty() || safe_relative_path(&manifest.name).is_none() {
        report.errors.push("theme name is invalid".to_string());
    }
    if let Err(error) = Version::parse(&manifest.version) {
        report
            .errors
            .push(format!("invalid theme version: {}", error));
    }
    match VersionReq::parse(&manifest.compatible_omnidoc) {
        Ok(requirement) => match Version::parse(env!("CARGO_PKG_VERSION")) {
            Ok(version) => {
                let compatible = requirement.matches(&version);
                report.compatible = Some(compatible);
                if !compatible {
                    report.errors.push(format!(
                        "theme requires OmniDoc {}, installed {}",
                        manifest.compatible_omnidoc, version
                    ));
                }
            }
            Err(error) => report
                .errors
                .push(format!("invalid OmniDoc version: {}", error)),
        },
        Err(error) => report.errors.push(format!(
            "invalid OmniDoc compatibility requirement: {}",
            error
        )),
    }
    if manifest
        .compatibility
        .as_deref()
        .is_some_and(|profile| profile.trim().is_empty())
    {
        report
            .errors
            .push("compatibility profile must not be empty".to_string());
    }

    if manifest.resources.html_css.is_empty()
        && manifest.resources.epub_css.is_empty()
        && manifest.resources.latex_packages.is_empty()
        && manifest.resources.latex_headers.is_empty()
        && manifest.resources.lua_filters.is_empty()
        && manifest.resources.templates.is_empty()
    {
        report
            .errors
            .push("theme bundle must declare at least one resource".to_string());
    }

    let canonical_library = fs::canonicalize(library).ok();
    for (kind, resources) in [
        ("html_css", &manifest.resources.html_css),
        ("epub_css", &manifest.resources.epub_css),
        ("latex_packages", &manifest.resources.latex_packages),
        ("latex_headers", &manifest.resources.latex_headers),
        ("lua_filters", &manifest.resources.lua_filters),
        ("templates", &manifest.resources.templates),
    ] {
        let mut seen = BTreeSet::new();
        for relative in resources {
            if !seen.insert(relative) {
                report
                    .errors
                    .push(format!("duplicate {} theme resource: {}", kind, relative));
                continue;
            }
            let Some(safe) = safe_relative_path(relative) else {
                report
                    .errors
                    .push(format!("unsafe theme resource path: {}", relative));
                continue;
            };
            let resolved = library.join(safe);
            match resolved.symlink_metadata() {
                Ok(metadata) if metadata.file_type().is_symlink() => report.errors.push(format!(
                    "theme resource must not be a symbolic link: {}",
                    relative
                )),
                Ok(metadata) if metadata.is_file() => {
                    if let (Some(root), Ok(canonical_resource)) =
                        (&canonical_library, fs::canonicalize(&resolved))
                    {
                        if !canonical_resource.starts_with(root) {
                            report.errors.push(format!(
                                "theme resource resolves outside the library: {}",
                                relative
                            ));
                        }
                    }
                }
                _ => report
                    .errors
                    .push(format!("missing theme resource: {}", relative)),
            }
        }
    }
    for font in &manifest.requirements.fonts {
        if font.trim().is_empty() {
            report
                .errors
                .push("font requirement must not be empty".to_string());
        }
    }
}

fn safe_relative_path(relative: &str) -> Option<PathBuf> {
    let path = Path::new(relative);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return None;
    }
    Some(path.to_path_buf())
}

fn print_reports(reports: &[ThemeReport], json: bool, detailed: bool) -> Result<()> {
    if json {
        let output = if detailed && reports.len() == 1 {
            serde_json::to_string_pretty(&reports[0])
        } else {
            serde_json::to_string_pretty(reports)
        }
        .map_err(|error| OmniDocError::Other(error.to_string()))?;
        println!("{}", output);
        return Ok(());
    }
    if reports.is_empty() {
        println!("No installed theme manifests found.");
        return Ok(());
    }
    for report in reports {
        let name = report
            .theme
            .as_ref()
            .map(|theme| theme.name.as_str())
            .unwrap_or(&report.manifest);
        let version = report
            .theme
            .as_ref()
            .map(|theme| theme.version.as_str())
            .unwrap_or("unknown");
        println!(
            "{} {} ({})",
            name,
            version,
            if report.valid { "valid" } else { "invalid" }
        );
        if detailed {
            if let Some(theme) = &report.theme {
                println!("  manifest: {}", report.manifest);
                println!(
                    "  compatibility: {}",
                    theme.compatibility.as_deref().unwrap_or("default")
                );
                println!("  OmniDoc: {}", theme.compatible_omnidoc);
                println!("  HTML CSS: {}", theme.resources.html_css.join(", "));
                println!("  EPUB CSS: {}", theme.resources.epub_css.join(", "));
                println!(
                    "  LaTeX packages: {}",
                    theme.resources.latex_packages.join(", ")
                );
                println!(
                    "  LaTeX headers: {}",
                    theme.resources.latex_headers.join(", ")
                );
                println!("  Lua filters: {}", theme.resources.lua_filters.join(", "));
                println!("  fonts: {}", theme.requirements.fonts.join(", "));
            }
        }
        for error in &report.errors {
            println!("  error: {}", error);
        }
    }
    Ok(())
}

fn ensure_valid(reports: &[ThemeReport]) -> Result<()> {
    if reports.is_empty() {
        return Err(OmniDocError::Other(
            "no installed theme manifests found".to_string(),
        ));
    }
    if reports.iter().all(|report| report.valid) {
        Ok(())
    } else {
        Err(OmniDocError::Other(
            "theme bundle validation failed".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::safe_relative_path;

    #[test]
    fn rejects_unsafe_theme_paths() {
        assert!(safe_relative_path("pandoc/css/theme.css").is_some());
        assert!(safe_relative_path("../theme.css").is_none());
        assert!(safe_relative_path("/theme.css").is_none());
    }
}
