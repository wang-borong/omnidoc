use crate::cli::commands::ThemeSubcommand;
use crate::cli::handlers::common::{create_config_manager, print_json_error};
use crate::config::schema::ConfigSchema;
use crate::config::{CliOverrides, MergedConfig};
use crate::error::{OmniDocError, Result};
use crate::extensions::{
    acquire_extension_store_read_locks, install_package, resolve_selected_theme,
    resolve_theme_manifest, resolve_theme_request, theme_catalog, uninstall_package,
    InstallPackageRequest, PackageKind, ResolvedPackageIdentity, ResolvedTheme, ThemeCatalogEntry,
};
use crate::project_tools;
use crate::utils::path;
use serde::Serialize;
use similar::TextDiff;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use toml_edit::{value, DocumentMut, Item, Table};

#[derive(Debug, Serialize)]
struct ResolvedThemeReport {
    id: String,
    name: String,
    version: String,
    description: Option<String>,
    category: Option<String>,
    recommended_for: Vec<String>,
    compatibility: Option<String>,
    outputs: Vec<String>,
    resources: ResolvedThemeResources,
    requirements: crate::extensions::ThemeRequirements,
    metadata: crate::extensions::ThemeMetadata,
    packages: Vec<ResolvedPackageReport>,
}

#[derive(Debug, Serialize)]
struct ResolvedThemeResources {
    html_css: Vec<String>,
    epub_css: Vec<String>,
    latex_packages: Vec<String>,
    latex_headers: Vec<String>,
    html_template: Option<String>,
    epub_template: Option<String>,
    latex_template: Option<String>,
    docx_reference_doc: Option<String>,
    pptx_reference_doc: Option<String>,
}

#[derive(Debug, Serialize)]
struct ResolvedPackageReport {
    kind: PackageKind,
    scope: crate::extensions::PackageScope,
    id: String,
    version: String,
    source: String,
    digest: String,
    root: String,
    compatible_pandoc: Option<String>,
}

#[derive(Debug, Serialize)]
struct ThemeValidationReport {
    package: ThemeCatalogEntry,
    resolved: bool,
    font_check_performed: bool,
    missing_fonts: Vec<String>,
    latex_check_performed: bool,
    missing_latex_packages: Vec<String>,
    errors: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ThemeApplyReport {
    schema_version: u32,
    path: String,
    id: String,
    version: String,
    compatibility: Option<String>,
    previous_id: Option<String>,
    previous_version: Option<String>,
    previous_compatibility: Option<String>,
    changed: bool,
    dry_run: bool,
    applied: bool,
    diff: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValidationOutcome {
    Valid,
    Invalid,
}

#[derive(Debug, Default)]
struct EnvironmentCheck {
    font_check_performed: bool,
    missing_fonts: Vec<String>,
    latex_check_performed: bool,
    missing_latex_packages: Vec<String>,
    errors: Vec<String>,
}

pub fn handle_theme(subcommand: ThemeSubcommand) -> Result<()> {
    let json = theme_json_mode(&subcommand);
    let mut validation_reported_failure = false;
    let result = match subcommand {
        ThemeSubcommand::Install {
            source,
            sha256,
            project,
            replace,
            json,
        } => install(&source, sha256.as_deref(), project, replace, json),
        ThemeSubcommand::Uninstall {
            package,
            project,
            json,
        } => uninstall(&package, project, json),
        ThemeSubcommand::List { project, json } => list(project, json),
        ThemeSubcommand::Inspect {
            package,
            project,
            json,
        } => inspect(&package, project, json),
        ThemeSubcommand::Validate {
            package,
            project,
            json,
            check_fonts,
            check_latex,
        } => match validate(package.as_deref(), project, check_fonts, check_latex, json) {
            Ok(ValidationOutcome::Valid) => Ok(()),
            Ok(ValidationOutcome::Invalid) => {
                validation_reported_failure = json;
                Err(OmniDocError::Project("theme validation failed".to_string()))
            }
            Err(error) => Err(error),
        },
        ThemeSubcommand::Apply {
            package,
            path,
            dry_run,
            diff,
            json,
        } => apply(&package, path, dry_run || diff, diff, json),
    };
    if let Err(error) = &result {
        if json && !validation_reported_failure {
            print_json_error(error);
        }
    }
    result
}

fn install(
    source: &str,
    sha256: Option<&str>,
    requested_project: Option<String>,
    replace: bool,
    json: bool,
) -> Result<()> {
    let project_root = explicit_project(requested_project)?;
    let config = load_config(project_root.as_deref())?;
    let _lock = project_root
        .as_deref()
        .map(|root| project_tools::acquire_project_write_lock(root, "install a theme package"))
        .transpose()?;
    let report = install_package(InstallPackageRequest {
        expected_kind: PackageKind::Theme,
        source,
        expected_sha256: sha256,
        project_root: project_root.as_deref(),
        config: &config,
        replace,
    })?;
    if json {
        print_json(&report)?;
    } else if report.installed {
        println!(
            "Installed theme {}@{} to {} ({}).",
            report.id, report.version, report.destination, report.digest
        );
    } else {
        println!(
            "Theme {}@{} is already installed with the same digest.",
            report.id, report.version
        );
    }
    Ok(())
}

fn uninstall(package: &str, requested_project: Option<String>, json: bool) -> Result<()> {
    let project_root = explicit_project(requested_project)?;
    let config = load_config(project_root.as_deref())?;
    let _lock = project_root
        .as_deref()
        .map(|root| project_tools::acquire_project_write_lock(root, "uninstall a theme package"))
        .transpose()?;
    let report = uninstall_package(
        PackageKind::Theme,
        package,
        project_root.as_deref(),
        &config,
    )?;
    if json {
        print_json(&report)?;
    } else {
        println!(
            "Uninstalled theme {}@{} from {}.",
            report.id, report.version, report.path
        );
    }
    Ok(())
}

fn list(requested_project: Option<String>, json: bool) -> Result<()> {
    let (project_root, config) = catalog_context(requested_project)?;
    let _extension_locks =
        acquire_extension_store_read_locks(project_root.as_deref(), &config, "list themes")?;
    let entries = theme_catalog(project_root.as_deref(), &config)?;
    if json {
        print_json(&entries)?;
    } else if entries.is_empty() {
        println!("No theme packages or built-in themes were found.");
    } else {
        for entry in entries {
            println!(
                "{}@{} [{}; {:?}] - {}",
                entry.id,
                entry.version,
                if entry.valid { "valid" } else { "invalid" },
                entry.scope,
                entry.name
            );
            if !entry.outputs.is_empty() {
                println!("  outputs: {}", entry.outputs.join(", "));
            }
            for error in entry.errors {
                println!("  error: {error}");
            }
        }
    }
    Ok(())
}

fn inspect(package: &str, requested_project: Option<String>, json: bool) -> Result<()> {
    let (project_root, config) = catalog_context(requested_project)?;
    let _extension_locks =
        acquire_extension_store_read_locks(project_root.as_deref(), &config, "inspect a theme")?;
    let theme = resolve_theme_request(project_root.as_deref(), &config, package)?;
    let report = resolved_theme_report(&theme);
    if json {
        print_json(&report)?;
    } else {
        println!("{}@{} - {}", report.id, report.version, report.name);
        if let Some(description) = report.description.as_deref() {
            println!("  {description}");
        }
        println!("  outputs: {}", report.outputs.join(", "));
        println!(
            "  inheritance: {}",
            report
                .packages
                .iter()
                .map(|package| format!("{}@{}", package.id, package.version))
                .collect::<Vec<_>>()
                .join(" -> ")
        );
        for css in &report.resources.html_css {
            println!("  HTML CSS: {css}");
        }
        for css in &report.resources.epub_css {
            println!("  EPUB CSS: {css}");
        }
        for header in &report.resources.latex_headers {
            println!("  LaTeX header: {header}");
        }
        for package in &report.packages {
            println!(
                "  package: {}@{} [{}; {}]",
                package.id, package.version, package.source, package.digest
            );
            if let Some(requirement) = package.compatible_pandoc.as_deref() {
                println!("    compatible Pandoc: {requirement}");
            }
        }
    }
    Ok(())
}

fn validate(
    requested_package: Option<&str>,
    requested_project: Option<String>,
    check_fonts: bool,
    check_latex: bool,
    json: bool,
) -> Result<ValidationOutcome> {
    let (project_root, config) = catalog_context(requested_project)?;
    let _extension_locks =
        acquire_extension_store_read_locks(project_root.as_deref(), &config, "validate themes")?;
    let catalog = theme_catalog(project_root.as_deref(), &config)?;
    let selected = if let Some(requested) = requested_package {
        let theme = resolve_theme_request(project_root.as_deref(), &config, requested)?;
        vec![matching_theme_entry(&catalog, &theme).ok_or_else(|| {
            OmniDocError::Other(format!(
                "resolved theme '{}' is missing from the catalog",
                theme.id
            ))
        })?]
    } else {
        catalog.iter().collect::<Vec<_>>()
    };
    let validate_resolved_request = requested_package.is_some();
    let mut reports = Vec::new();
    let mut failed = false;
    for entry in selected {
        let resolved = if validate_resolved_request {
            resolve_theme_request(
                project_root.as_deref(),
                &config,
                &format!("{}@={}", entry.id, entry.version),
            )
        } else {
            resolve_theme_manifest(
                project_root.as_deref(),
                &config,
                Path::new(&entry.manifest_path),
            )
        };
        let mut environment = EnvironmentCheck::default();
        let mut resolution_ok = false;
        if entry.valid {
            match resolved {
                Ok(theme) => {
                    resolution_ok = true;
                    environment = check_environment(&theme, check_fonts, check_latex);
                }
                Err(error) => environment.errors.push(error.to_string()),
            }
        }
        let mut errors = entry.errors.clone();
        errors.extend(environment.errors.clone());
        failed |= !entry.valid || !resolution_ok || !errors.is_empty();
        reports.push(ThemeValidationReport {
            package: entry.clone(),
            resolved: resolution_ok,
            font_check_performed: environment.font_check_performed,
            missing_fonts: environment.missing_fonts,
            latex_check_performed: environment.latex_check_performed,
            missing_latex_packages: environment.missing_latex_packages,
            errors,
        });
    }
    if json {
        print_json(&reports)?;
    } else if reports.is_empty() {
        println!("No theme packages or built-in themes were found.");
    } else {
        for report in &reports {
            let valid = report.package.valid && report.resolved && report.errors.is_empty();
            println!(
                "{} {}@{}",
                if valid { "ok" } else { "fail" },
                report.package.id,
                report.package.version
            );
            for error in &report.errors {
                println!("  {error}");
            }
        }
    }
    Ok(if failed {
        ValidationOutcome::Invalid
    } else {
        ValidationOutcome::Valid
    })
}

fn apply(
    requested: &str,
    requested_path: Option<String>,
    dry_run: bool,
    include_diff: bool,
    json: bool,
) -> Result<()> {
    let project_root = path::determine_project_root(requested_path)?;
    let _lock = (!dry_run)
        .then(|| project_tools::acquire_project_write_lock(&project_root, "apply a theme"))
        .transpose()?;
    let config = load_config(Some(&project_root))?;
    let _extension_locks =
        acquire_extension_store_read_locks(Some(&project_root), &config, "apply a theme")?;
    let theme = resolve_theme_request(Some(&project_root), &config, requested)?;
    let config_path = project_root.join(".omnidoc.toml");
    let existed = config_path.is_file();
    let original = if existed {
        fs::read_to_string(&config_path)?
    } else {
        String::new()
    };
    let mut document = if original.trim().is_empty() {
        DocumentMut::new()
    } else {
        original.parse::<DocumentMut>().map_err(|error| {
            OmniDocError::Config(format!(
                "failed to parse {}: {error}",
                config_path.display()
            ))
        })?
    };
    if !document.as_table().contains_key("theme") {
        document
            .as_table_mut()
            .insert("theme", Item::Table(Table::new()));
    }
    let theme_table = document
        .get_mut("theme")
        .and_then(Item::as_table_mut)
        .ok_or_else(|| {
            OmniDocError::Config("configuration key 'theme' must be a table".to_string())
        })?;
    let previous_id = theme_table
        .get("name")
        .and_then(Item::as_str)
        .map(str::to_string);
    let previous_version = theme_table
        .get("version")
        .and_then(Item::as_str)
        .map(str::to_string);
    let previous_compatibility = theme_table
        .get("compatibility")
        .and_then(Item::as_str)
        .map(str::to_string);
    let exact_version = format!("={}", theme.version);
    let compatibility = theme.compatibility.clone();
    theme_table.insert("name", value(&theme.id));
    theme_table.insert("version", value(&exact_version));
    if let Some(compatibility) = compatibility.as_deref() {
        theme_table.insert("compatibility", value(compatibility));
    } else {
        theme_table.remove("compatibility");
    }
    let updated = document.to_string();
    toml::from_str::<ConfigSchema>(&updated).map_err(|error| {
        OmniDocError::Config(format!(
            "theme selection is not valid for {}: {error}",
            config_path.display()
        ))
    })?;
    let changed = updated != original;
    let diff =
        (include_diff && changed).then(|| config_diff(&config_path, existed, &original, &updated));
    let mut report = ThemeApplyReport {
        schema_version: 1,
        path: config_path.to_string_lossy().to_string(),
        id: theme.id,
        version: exact_version,
        compatibility,
        previous_id,
        previous_version,
        previous_compatibility,
        changed,
        dry_run,
        applied: false,
        diff,
    };
    if changed && !dry_run {
        crate::utils::fs::atomic_write(&config_path, updated.as_bytes())?;
        create_config_manager(Some(&project_root), CliOverrides::new())?;
        report.applied = true;
    }
    if json {
        print_json(&report)?;
    } else if !changed {
        println!(
            "Theme {}@{} is already selected in {}.",
            report.id,
            report.version.trim_start_matches('='),
            report.path
        );
    } else if dry_run {
        println!(
            "Would apply theme {}@{} in {}.",
            report.id,
            report.version.trim_start_matches('='),
            report.path
        );
        if let Some(diff) = report.diff.as_deref() {
            print!("{diff}");
        }
        println!("No files were changed.");
    } else {
        println!(
            "Applied theme {}@{} in {}.",
            report.id,
            report.version.trim_start_matches('='),
            report.path
        );
    }
    Ok(())
}

pub(crate) fn theme_diagnostic(
    project_root: &Path,
    config: &MergedConfig,
    check_fonts: bool,
    check_latex: bool,
) -> (bool, String) {
    let theme = match resolve_selected_theme(Some(project_root), config) {
        Ok(Some(theme)) => theme,
        Ok(None) => return (true, "no theme selected".to_string()),
        Err(error) => return (false, error.to_string()),
    };
    let applies_to_latex = theme.supports_output("pdf") || theme.supports_output("latex");
    let environment = check_environment(
        &theme,
        check_fonts && applies_to_latex,
        check_latex && applies_to_latex,
    );
    if !environment.errors.is_empty() {
        return (false, environment.errors.join("; "));
    }
    (
        true,
        format!(
            "version {}; {} package(s) in inheritance chain; {} fonts and {} system LaTeX packages declared",
            theme.version,
            theme.packages.len(),
            theme.requirements.fonts.len(),
            theme.requirements.system_latex_packages.len()
        ),
    )
}

fn check_environment(
    theme: &ResolvedTheme,
    check_fonts: bool,
    check_latex: bool,
) -> EnvironmentCheck {
    let mut report = EnvironmentCheck::default();
    if check_fonts && !theme.requirements.fonts.is_empty() {
        report.font_check_performed = true;
        for font in &theme.requirements.fonts {
            let output = match Command::new("fc-match")
                .args(["--format", "%{family}\n", "--", font])
                .output()
            {
                Ok(output) => output,
                Err(error) => {
                    report.errors.push(format!(
                        "cannot check theme fonts because fc-match failed: {error}"
                    ));
                    break;
                }
            };
            let families = String::from_utf8_lossy(&output.stdout);
            if !output.status.success() || !font_family_matches(font, &families) {
                report.missing_fonts.push(font.clone());
                report.errors.push(format!(
                    "required font '{}' is not installed (fontconfig matched '{}')",
                    font,
                    families.lines().next().unwrap_or("unknown").trim()
                ));
            }
        }
    }
    if check_latex && !theme.requirements.system_latex_packages.is_empty() {
        report.latex_check_performed = true;
        for package in &theme.requirements.system_latex_packages {
            if !valid_latex_package_name(package) {
                report
                    .errors
                    .push(format!("invalid system LaTeX package name: {package}"));
                continue;
            }
            let file = format!("{package}.sty");
            let output = match Command::new("kpsewhich").args(["--", &file]).output() {
                Ok(output) => output,
                Err(error) => {
                    report.errors.push(format!(
                        "cannot check LaTeX packages because kpsewhich failed: {error}"
                    ));
                    break;
                }
            };
            if !output.status.success() || String::from_utf8_lossy(&output.stdout).trim().is_empty()
            {
                report.missing_latex_packages.push(package.clone());
                report.errors.push(format!(
                    "required system LaTeX package '{}' is not installed",
                    package
                ));
            }
        }
    }
    report
}

fn resolved_theme_report(theme: &ResolvedTheme) -> ResolvedThemeReport {
    ResolvedThemeReport {
        id: theme.id.clone(),
        name: theme.name.clone(),
        version: theme.version.clone(),
        description: theme.description.clone(),
        category: theme.category.clone(),
        recommended_for: theme.recommended_for.clone(),
        compatibility: theme.compatibility.clone(),
        outputs: theme.outputs.clone(),
        resources: ResolvedThemeResources {
            html_css: display_paths(&theme.resources.html_css),
            epub_css: display_paths(&theme.resources.epub_css),
            latex_packages: display_paths(&theme.resources.latex_packages),
            latex_headers: display_paths(&theme.resources.latex_headers),
            html_template: display_optional_path(theme.resources.html_template.as_deref()),
            epub_template: display_optional_path(theme.resources.epub_template.as_deref()),
            latex_template: display_optional_path(theme.resources.latex_template.as_deref()),
            docx_reference_doc: display_optional_path(
                theme.resources.docx_reference_doc.as_deref(),
            ),
            pptx_reference_doc: display_optional_path(
                theme.resources.pptx_reference_doc.as_deref(),
            ),
        },
        requirements: theme.requirements.clone(),
        metadata: theme.metadata.clone(),
        packages: theme.packages.iter().map(resolved_package_report).collect(),
    }
}

fn resolved_package_report(package: &ResolvedPackageIdentity) -> ResolvedPackageReport {
    ResolvedPackageReport {
        kind: package.kind,
        scope: package.scope,
        id: package.id.clone(),
        version: package.version.clone(),
        source: package.source.clone(),
        digest: package.digest.clone(),
        root: package.root.to_string_lossy().to_string(),
        compatible_pandoc: package.compatible_pandoc.clone(),
    }
}

fn matching_theme_entry<'a>(
    catalog: &'a [ThemeCatalogEntry],
    theme: &ResolvedTheme,
) -> Option<&'a ThemeCatalogEntry> {
    let package = theme.packages.last()?;
    catalog.iter().find(|entry| {
        entry.id == theme.id
            && entry.version == theme.version
            && entry.digest.as_deref() == Some(package.digest.as_str())
    })
}

fn explicit_project(requested: Option<String>) -> Result<Option<PathBuf>> {
    requested
        .map(|path| path::determine_project_root(Some(path)))
        .transpose()
}

fn catalog_context(requested: Option<String>) -> Result<(Option<PathBuf>, MergedConfig)> {
    let project_root = match requested {
        Some(path) => Some(path::determine_project_root(Some(path))?),
        None => {
            let current = std::env::current_dir()?;
            path::locate_project_root(&current)
        }
    };
    let config = load_config(project_root.as_deref())?;
    Ok((project_root, config))
}

fn load_config(project_root: Option<&Path>) -> Result<MergedConfig> {
    Ok(create_config_manager(project_root, CliOverrides::new())?
        .get_merged()
        .clone())
}

fn display_paths(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect()
}

fn display_optional_path(path: Option<&Path>) -> Option<String> {
    path.map(|path| path.to_string_lossy().to_string())
}

fn config_diff(path: &Path, existed: bool, before: &str, after: &str) -> String {
    let path = path.to_string_lossy();
    let old_label = if existed {
        format!("a/{path}")
    } else {
        "/dev/null".to_string()
    };
    let new_label = format!("b/{path}");
    TextDiff::from_lines(before, after)
        .unified_diff()
        .header(&old_label, &new_label)
        .to_string()
}

pub(crate) fn font_family_matches(requested: &str, families: &str) -> bool {
    families
        .lines()
        .flat_map(|line| line.split(','))
        .any(|family| family.trim().eq_ignore_ascii_case(requested.trim()))
}

pub(crate) fn valid_latex_package_name(package: &str) -> bool {
    !package.is_empty()
        && package
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

fn theme_json_mode(command: &ThemeSubcommand) -> bool {
    match command {
        ThemeSubcommand::Install { json, .. }
        | ThemeSubcommand::Uninstall { json, .. }
        | ThemeSubcommand::List { json, .. }
        | ThemeSubcommand::Inspect { json, .. }
        | ThemeSubcommand::Validate { json, .. }
        | ThemeSubcommand::Apply { json, .. } => *json,
    }
}

fn print_json(value: &impl Serialize) -> Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(value)
            .map_err(|error| OmniDocError::Other(error.to_string()))?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{font_family_matches, valid_latex_package_name};

    #[test]
    fn distinguishes_requested_fonts_from_fontconfig_fallbacks() {
        assert!(font_family_matches(
            "Noto Serif CJK SC",
            "Noto Serif CJK SC,Noto Serif CJK TC\n"
        ));
        assert!(!font_family_matches("OmniDoc Missing Font", "Noto Sans\n"));
    }

    #[test]
    fn validates_latex_package_names_before_invoking_kpsewhich() {
        assert!(valid_latex_package_name("tcolorbox"));
        assert!(valid_latex_package_name("xeCJK"));
        assert!(!valid_latex_package_name("../outside"));
        assert!(!valid_latex_package_name("package.sty"));
    }
}
