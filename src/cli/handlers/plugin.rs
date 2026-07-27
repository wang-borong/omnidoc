use crate::cli::handlers::common::{create_config_manager, print_json_error};
use crate::config::CliOverrides;
use crate::error::{OmniDocError, Result};
use crate::project_tools;
use crate::utils::directories::data_local_dir;
use crate::utils::path;
use serde::Serialize;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;

const PLUGIN_EXAMPLES_DIR: &str = "plugin-examples";

#[derive(Debug, Serialize)]
struct PluginExampleInfo {
    preset: String,
    key: String,
    name: Option<String>,
    version: Option<String>,
    description: Option<String>,
    hooks: Vec<String>,
    valid: bool,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct PluginInstallReport {
    schema_version: u32,
    preset: String,
    source: String,
    destination: String,
    files: Vec<String>,
    dry_run: bool,
    installed: bool,
}

pub fn handle_plugin(path: Option<String>, json: bool, validate: bool) -> Result<()> {
    let plugins = match (|| {
        let project_path = path::determine_project_context(path)?;
        let config_manager = create_config_manager(Some(&project_path), CliOverrides::new())?;
        Ok(project_tools::discovered_plugins(
            &project_path,
            config_manager.get_merged(),
        ))
    })() {
        Ok(plugins) => plugins,
        Err(error) => {
            if json {
                print_json_error(&error);
            }
            return Err(error);
        }
    };
    if json {
        let content = serde_json::to_string_pretty(&plugins)
            .map_err(|err| OmniDocError::Other(err.to_string()))?;
        println!("{}", content);
    } else if plugins.is_empty() {
        println!("No project plugins or external templates discovered.");
        println!("Run `omnidoc plugin examples` to see installable examples.");
    } else {
        for plugin in &plugins {
            let status = if plugin.valid { "ok" } else { "fail" };
            let hooks = if plugin.hooks.is_empty() {
                "no hooks".to_string()
            } else {
                plugin.hooks.join(", ")
            };
            if let Some(error) = &plugin.error {
                println!(
                    "{} {} ({}) [{}] - {}",
                    status, plugin.key, plugin.path, hooks, error
                );
            } else {
                println!("{} {} ({}) [{}]", status, plugin.key, plugin.path, hooks);
            }
        }
    }

    if validate && plugins.iter().any(|plugin| !plugin.valid) {
        return Err(OmniDocError::Project(
            "plugin validation failed".to_string(),
        ));
    }
    Ok(())
}

pub fn handle_plugin_examples(path: Option<String>, json: bool) -> Result<()> {
    let result = (|| {
        let context = path::determine_project_context(path)?;
        let root = configured_examples_root(&context)?;
        load_examples(&root)
    })();
    let examples = match result {
        Ok(examples) => examples,
        Err(error) => {
            if json {
                print_json_error(&error);
            }
            return Err(error);
        }
    };

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&examples)
                .map_err(|error| OmniDocError::Other(error.to_string()))?
        );
    } else if examples.is_empty() {
        println!("No bundled plugin examples found.");
    } else {
        for example in examples {
            let status = if example.valid {
                "available"
            } else {
                "invalid"
            };
            let description = example.description.as_deref().unwrap_or("No description");
            println!("{} [{}] - {}", example.preset, status, description);
            if !example.hooks.is_empty() {
                println!("  hooks: {}", example.hooks.join(", "));
            }
            if let Some(error) = example.error {
                println!("  error: {}", error);
            }
        }
        println!("\nInstall one with `omnidoc plugin add <PRESET> [PATH]`.");
    }
    Ok(())
}

pub fn handle_plugin_add(
    preset: String,
    path: Option<String>,
    dry_run: bool,
    json: bool,
) -> Result<()> {
    let result = install_plugin_example(&preset, path, dry_run);
    let report = match result {
        Ok(report) => report,
        Err(error) => {
            if json {
                print_json_error(&error);
            }
            return Err(error);
        }
    };

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|error| OmniDocError::Other(error.to_string()))?
        );
    } else if report.dry_run {
        println!(
            "Would install plugin example '{}' to {} ({} files).",
            report.preset,
            report.destination,
            report.files.len()
        );
    } else {
        println!(
            "Installed plugin example '{}' to {} ({} files).",
            report.preset,
            report.destination,
            report.files.len()
        );
        println!("Next: run `omnidoc plugin validate` and inspect the plugin README.");
    }
    Ok(())
}

fn install_plugin_example(
    requested_preset: &str,
    requested_path: Option<String>,
    dry_run: bool,
) -> Result<PluginInstallReport> {
    let preset = normalize_plugin_key(requested_preset);
    if !valid_plugin_key(&preset) {
        return Err(OmniDocError::Project(format!(
            "invalid plugin example key: {requested_preset}"
        )));
    }
    let project_path = path::determine_project_root(requested_path)?;
    let examples_root = configured_examples_root(&project_path)?;
    let examples = load_examples(&examples_root)?;
    let example = examples
        .iter()
        .find(|example| normalize_plugin_key(&example.preset) == preset)
        .ok_or_else(|| {
            let available = examples
                .iter()
                .map(|example| example.preset.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            OmniDocError::Project(format!(
                "unknown plugin example '{requested_preset}'; available examples: {available}"
            ))
        })?;
    if !example.valid {
        return Err(OmniDocError::Project(format!(
            "plugin example '{}' is invalid: {}",
            example.preset,
            example.error.as_deref().unwrap_or("validation failed")
        )));
    }

    let source = examples_root.join(&example.preset);
    let files = example_files(&source)?;
    let plugins_dir = project_path.join("plugins");
    reject_symlink(&plugins_dir, "project plugins directory")?;
    let destination_key = normalize_plugin_key(&example.key);
    if !valid_plugin_key(&destination_key) {
        return Err(OmniDocError::Project(format!(
            "plugin example '{}' has an unsafe manifest key: {}",
            example.preset, example.key
        )));
    }
    let destination = plugins_dir.join(&destination_key);
    if destination.exists() {
        return Err(OmniDocError::Project(format!(
            "plugin destination already exists: {}",
            destination.display()
        )));
    }

    let report_files = files
        .iter()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .collect::<Vec<_>>();
    let mut report = PluginInstallReport {
        schema_version: 1,
        preset: example.preset.clone(),
        source: source.display().to_string(),
        destination: destination.display().to_string(),
        files: report_files,
        dry_run,
        installed: false,
    };
    if dry_run {
        return Ok(report);
    }

    let _project_lock = project_tools::acquire_project_write_lock(
        &project_path,
        "install a project plugin example",
    )?;
    reject_symlink(&plugins_dir, "project plugins directory")?;
    if destination.exists() {
        return Err(OmniDocError::Project(format!(
            "plugin destination already exists: {}",
            destination.display()
        )));
    }
    fs::create_dir_all(&plugins_dir).map_err(OmniDocError::Io)?;
    let canonical_plugins = fs::canonicalize(&plugins_dir).map_err(OmniDocError::Io)?;
    if !canonical_plugins.starts_with(&project_path) {
        return Err(OmniDocError::Project(format!(
            "project plugins directory resolves outside the project: {}",
            plugins_dir.display()
        )));
    }

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let staging = plugins_dir.join(format!(
        ".omnidoc-install-{}-{}-{nonce}",
        destination_key,
        std::process::id()
    ));
    if staging.exists() {
        return Err(OmniDocError::Project(format!(
            "temporary plugin installation path already exists: {}",
            staging.display()
        )));
    }
    fs::create_dir(&staging).map_err(OmniDocError::Io)?;
    let copy_result = copy_example_files(&source, &staging, &files);
    if let Err(error) = copy_result {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    let staged_info = project_tools::inspect_plugin_manifest(&staging.join("manifest.toml"));
    if !staged_info.valid || normalize_plugin_key(&staged_info.key) != destination_key {
        let detail = staged_info
            .error
            .as_deref()
            .unwrap_or("copied manifest key changed during installation");
        let _ = fs::remove_dir_all(&staging);
        return Err(OmniDocError::Project(format!(
            "copied plugin example failed validation: {detail}"
        )));
    }
    if destination.exists() {
        let _ = fs::remove_dir_all(&staging);
        return Err(OmniDocError::Project(format!(
            "plugin destination was created during installation: {}",
            destination.display()
        )));
    }
    if let Err(error) = fs::rename(&staging, &destination) {
        let _ = fs::remove_dir_all(&staging);
        return Err(OmniDocError::Io(error));
    }
    report.installed = true;
    Ok(report)
}

fn configured_examples_root(project_path: &Path) -> Result<PathBuf> {
    let manager = create_config_manager(Some(project_path), CliOverrides::new())?;
    let library = manager
        .get_merged()
        .lib_path
        .as_ref()
        .map(PathBuf::from)
        .or_else(|| data_local_dir().map(|path| path.join("omnidoc")))
        .ok_or_else(|| OmniDocError::Project("OmniDoc library path is unavailable".to_string()))?;
    let root = library.join(PLUGIN_EXAMPLES_DIR);
    if !root.is_dir() {
        return Err(OmniDocError::Project(format!(
            "bundled plugin examples were not found in {}; run `omnidoc lib update`",
            root.display()
        )));
    }
    reject_symlink(&root, "plugin examples directory")?;
    Ok(root)
}

fn load_examples(root: &Path) -> Result<Vec<PluginExampleInfo>> {
    let mut entries = fs::read_dir(root)
        .map_err(OmniDocError::Io)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());
    let mut examples = Vec::new();
    for entry in entries {
        let preset = entry.file_name().to_string_lossy().to_string();
        let directory = entry.path();
        let manifest = directory.join("manifest.toml");
        let mut info = project_tools::inspect_plugin_manifest(&manifest);
        if info.valid {
            let normalized_key = normalize_plugin_key(&info.key);
            if !valid_plugin_key(&normalized_key) {
                info.valid = false;
                info.error = Some(format!("manifest key '{}' is unsafe", info.key));
            } else if normalized_key != normalize_plugin_key(&preset) {
                info.valid = false;
                info.error = Some(format!(
                    "manifest key '{}' does not match example directory '{}'",
                    info.key, preset
                ));
            }
        }
        examples.push(PluginExampleInfo {
            preset,
            key: info.key,
            name: info.name,
            version: info.version,
            description: info.description,
            hooks: info.hooks,
            valid: info.valid,
            error: info.error,
        });
    }
    Ok(examples)
}

fn example_files(source: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in WalkDir::new(source).follow_links(false) {
        let entry = entry.map_err(|error| OmniDocError::Project(error.to_string()))?;
        if entry.file_type().is_symlink() {
            return Err(OmniDocError::Project(format!(
                "plugin example contains a symbolic link: {}",
                entry.path().display()
            )));
        }
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry.path().strip_prefix(source).map_err(|error| {
            OmniDocError::Project(format!("invalid plugin example path: {error}"))
        })?;
        if relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(OmniDocError::Project(format!(
                "unsafe plugin example path: {}",
                relative.display()
            )));
        }
        files.push(relative.to_path_buf());
    }
    files.sort();
    if !files.iter().any(|path| path == Path::new("manifest.toml")) {
        return Err(OmniDocError::Project(format!(
            "plugin example is missing manifest.toml: {}",
            source.display()
        )));
    }
    Ok(files)
}

fn copy_example_files(source: &Path, staging: &Path, files: &[PathBuf]) -> Result<()> {
    for relative in files {
        let destination = staging.join(relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(OmniDocError::Io)?;
        }
        fs::copy(source.join(relative), destination).map_err(OmniDocError::Io)?;
    }
    Ok(())
}

fn reject_symlink(path: &Path, label: &str) -> Result<()> {
    if path
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err(OmniDocError::Project(format!(
            "{label} must not be a symbolic link: {}",
            path.display()
        )));
    }
    Ok(())
}

fn normalize_plugin_key(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace('_', "-")
}

fn valid_plugin_key(value: &str) -> bool {
    let mut characters = value.chars();
    characters
        .next()
        .is_some_and(|character| character.is_ascii_alphanumeric())
        && characters.all(|character| character.is_ascii_alphanumeric() || character == '-')
}

#[cfg(test)]
mod tests {
    use super::{normalize_plugin_key, valid_plugin_key};

    #[test]
    fn normalizes_and_validates_plugin_keys() {
        assert_eq!(normalize_plugin_key("Quality_Gate"), "quality-gate");
        assert!(valid_plugin_key("quality-gate"));
        assert!(!valid_plugin_key("../quality-gate"));
        assert!(!valid_plugin_key(""));
    }
}
