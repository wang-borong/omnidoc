use crate::cli::commands::PluginSubcommand;
use crate::cli::handlers::common::{create_config_manager, print_json_error};
use crate::cli::handlers::config::handle_project_config_set_locked;
use crate::config::{CliOverrides, MergedConfig};
use crate::error::{OmniDocError, Result};
use crate::extensions::{
    acquire_extension_store_read_locks, ensure_pandoc_compatible, install_package,
    is_plugin_trusted, package_spec, plugin_catalog, resolve_plugin_manifest,
    resolve_plugin_request, revoke_plugin_trust, run_plugin_command, trust_plugin,
    uninstall_package, validate_plugin_lua, InstallPackageRequest, PackageKind, PluginCatalogEntry,
    ResolvedPlugin, PACKAGE_MANIFEST_FILE,
};
use crate::project_tools;
use crate::utils::directories::data_local_dir;
use crate::utils::path;
use serde::Serialize;
use std::path::{Component, Path, PathBuf};

const PLUGIN_EXAMPLES_DIR: &str = "plugin-examples";

#[derive(Debug, Serialize)]
struct PluginTrustReport {
    schema_version: u32,
    id: String,
    version: String,
    digest: String,
    trusted: bool,
    changed: bool,
}

#[derive(Debug, Serialize)]
struct PluginValidationReport {
    package: PluginCatalogEntry,
    lua_checked: bool,
    lua_valid: Option<bool>,
    errors: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValidationOutcome {
    Valid,
    Invalid,
}

pub fn handle_plugin(subcommand: PluginSubcommand) -> Result<()> {
    let json = plugin_json_mode(&subcommand);
    let delegated_json_errors = matches!(
        &subcommand,
        PluginSubcommand::Enable { .. } | PluginSubcommand::Disable { .. }
    );
    let mut validation_reported_failure = false;
    let result = match subcommand {
        PluginSubcommand::Install {
            source,
            sha256,
            project,
            replace,
            json,
        } => install(&source, sha256.as_deref(), project, replace, json),
        PluginSubcommand::InstallExample {
            preset,
            project,
            replace,
            json,
        } => install_example(&preset, project, replace, json),
        PluginSubcommand::Uninstall {
            package,
            project,
            json,
        } => uninstall(&package, project, json),
        PluginSubcommand::List { project, json } => list(project, json),
        PluginSubcommand::Inspect {
            package,
            project,
            json,
        } => inspect(&package, project, json),
        PluginSubcommand::Validate {
            package,
            project,
            check_lua,
            json,
        } => match validate(package.as_deref(), project, check_lua, json) {
            Ok(ValidationOutcome::Valid) => Ok(()),
            Ok(ValidationOutcome::Invalid) => {
                validation_reported_failure = json;
                Err(OmniDocError::Project(
                    "plugin validation failed".to_string(),
                ))
            }
            Err(error) => Err(error),
        },
        PluginSubcommand::Enable {
            package,
            path,
            json,
        } => set_enabled(&package, path, true, json),
        PluginSubcommand::Disable {
            package,
            path,
            json,
        } => set_enabled(&package, path, false, json),
        PluginSubcommand::Trust {
            package,
            project,
            json,
        } => set_trust(&package, project, true, json),
        PluginSubcommand::Untrust {
            package,
            project,
            json,
        } => set_trust(&package, project, false, json),
        PluginSubcommand::Run {
            package,
            command,
            project,
            arguments,
        } => run(&package, &command, project, &arguments),
    };
    if let Err(error) = &result {
        if json && !delegated_json_errors && !validation_reported_failure {
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
        .map(|root| project_tools::acquire_project_write_lock(root, "install a plugin package"))
        .transpose()?;
    let report = install_package(InstallPackageRequest {
        expected_kind: PackageKind::Plugin,
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
            "Installed plugin {}@{} to {} ({}).",
            report.id, report.version, report.destination, report.digest
        );
        println!("The plugin is inert until it is trusted and enabled for a project.");
    } else {
        println!(
            "Plugin {}@{} is already installed with the same digest.",
            report.id, report.version
        );
    }
    Ok(())
}

fn install_example(
    preset: &str,
    requested_project: Option<String>,
    replace: bool,
    json: bool,
) -> Result<()> {
    if !safe_example_key(preset) {
        return Err(OmniDocError::Project(format!(
            "invalid plugin example key: {preset}"
        )));
    }
    let project_root = path::determine_project_root(requested_project)?;
    let config = load_config(Some(&project_root))?;
    let library = config
        .lib_path
        .as_ref()
        .map(PathBuf::from)
        .or_else(|| data_local_dir().map(|root| root.join("omnidoc")))
        .ok_or_else(|| OmniDocError::Project("OmniDoc library path is unavailable".to_string()))?;
    let source = library.join(PLUGIN_EXAMPLES_DIR).join(preset);
    if !source.join(PACKAGE_MANIFEST_FILE).is_file() {
        let available = bundled_example_names(&library.join(PLUGIN_EXAMPLES_DIR));
        return Err(OmniDocError::Project(format!(
            "unknown plugin example '{preset}'; available examples: {}",
            if available.is_empty() {
                "none (run `omnidoc lib update`)".to_string()
            } else {
                available.join(", ")
            }
        )));
    }
    let _lock =
        project_tools::acquire_project_write_lock(&project_root, "install a plugin example")?;
    let source_string = source.to_string_lossy().to_string();
    let report = install_package(InstallPackageRequest {
        expected_kind: PackageKind::Plugin,
        source: &source_string,
        expected_sha256: None,
        project_root: Some(&project_root),
        config: &config,
        replace,
    })?;
    if json {
        print_json(&report)?;
    } else if report.installed {
        println!(
            "Installed plugin example '{}' as {}@{}.",
            preset, report.id, report.version
        );
        println!(
            "Next: `omnidoc plugin trust {}@={} --project {}` then `omnidoc plugin enable {}@={} {}`.",
            report.id,
            report.version,
            project_root.display(),
            report.id,
            report.version,
            project_root.display()
        );
    } else {
        println!(
            "Plugin example '{}' is already installed as {}@{}.",
            preset, report.id, report.version
        );
    }
    Ok(())
}

fn uninstall(package: &str, requested_project: Option<String>, json: bool) -> Result<()> {
    let project_root = explicit_project(requested_project)?;
    let config = load_config(project_root.as_deref())?;
    let _lock = project_root
        .as_deref()
        .map(|root| project_tools::acquire_project_write_lock(root, "uninstall a plugin package"))
        .transpose()?;
    let report = uninstall_package(
        PackageKind::Plugin,
        package,
        project_root.as_deref(),
        &config,
    )?;
    if json {
        print_json(&report)?;
    } else {
        println!(
            "Uninstalled plugin {}@{} from {}.",
            report.id, report.version, report.path
        );
    }
    Ok(())
}

fn list(requested_project: Option<String>, json: bool) -> Result<()> {
    let (project_root, config) = catalog_context(requested_project)?;
    let _extension_locks =
        acquire_extension_store_read_locks(project_root.as_deref(), &config, "list plugins")?;
    let entries = plugin_catalog(project_root.as_deref(), &config)?;
    if json {
        print_json(&entries)?;
    } else if entries.is_empty() {
        println!("No plugin packages are installed.");
    } else {
        for entry in entries {
            let state = match (entry.enabled, entry.trusted, entry.valid) {
                (_, _, false) => "invalid",
                (true, true, true) => "enabled, trusted",
                (true, false, true) => "enabled, untrusted",
                (false, true, true) => "disabled, trusted",
                (false, false, true) => "disabled, untrusted",
            };
            println!(
                "{}@{} [{}; {:?}] - {}",
                entry.id, entry.version, state, entry.scope, entry.name
            );
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
        acquire_extension_store_read_locks(project_root.as_deref(), &config, "inspect a plugin")?;
    let plugin = resolve_plugin_request(project_root.as_deref(), &config, package)?;
    let entry = matching_catalog_entry(project_root.as_deref(), &config, &plugin)?;
    if json {
        print_json(&entry)?;
    } else {
        println!("{}@{} - {}", entry.id, entry.version, entry.name);
        if let Some(description) = entry.description.as_deref() {
            println!("  {description}");
        }
        println!("  source: {} ({:?})", entry.source, entry.scope);
        println!("  digest: {}", entry.digest.as_deref().unwrap_or("unknown"));
        println!(
            "  compatible Pandoc: {}",
            entry.compatible_pandoc.as_deref().unwrap_or("not declared")
        );
        println!("  enabled: {}", entry.enabled);
        println!("  trusted: {}", entry.trusted);
        for filter in entry.filters {
            let formats = if filter.formats.is_empty() {
                "all formats".to_string()
            } else {
                filter.formats.join(", ")
            };
            println!(
                "  filter: {} (order {}, {})",
                filter.script, filter.order, formats
            );
            if let Some(key) = filter.dependency_key.as_deref() {
                println!("    dependency key: {key}");
            }
        }
        for command in entry.commands {
            println!("  command: {} -> {}", command.name, command.script);
        }
    }
    Ok(())
}

fn validate(
    requested_package: Option<&str>,
    requested_project: Option<String>,
    check_lua: bool,
    json: bool,
) -> Result<ValidationOutcome> {
    let (project_root, config) = catalog_context(requested_project)?;
    let _extension_locks =
        acquire_extension_store_read_locks(project_root.as_deref(), &config, "validate plugins")?;
    let catalog = plugin_catalog(project_root.as_deref(), &config)?;
    let selected = if let Some(requested) = requested_package {
        let plugin = resolve_plugin_request(project_root.as_deref(), &config, requested)?;
        vec![matching_entry(&catalog, &plugin).ok_or_else(|| {
            OmniDocError::Other(format!(
                "resolved plugin '{}' is missing from the catalog",
                plugin.id
            ))
        })?]
    } else {
        catalog.iter().collect::<Vec<_>>()
    };
    let validate_resolved_request = requested_package.is_some();
    let mut reports = Vec::new();
    let mut failed = false;
    for entry in selected {
        let mut errors = entry.errors.clone();
        let mut lua_valid = None;
        let mut lua_was_checked = false;
        if entry.valid {
            let identity = format!("{}@={}", entry.id, entry.version);
            let resolved = if validate_resolved_request {
                resolve_plugin_request(project_root.as_deref(), &config, &identity)
            } else {
                resolve_plugin_manifest(
                    project_root.as_deref(),
                    &config,
                    Path::new(&entry.manifest_path),
                )
            };
            match resolved {
                Ok(plugin) => {
                    if let Err(error) =
                        ensure_pandoc_compatible(std::slice::from_ref(&plugin.package), &config)
                    {
                        errors.push(error.to_string());
                    } else if check_lua {
                        lua_was_checked = true;
                        match validate_plugin_lua(&plugin, &config) {
                            Ok(()) => lua_valid = Some(true),
                            Err(error) => {
                                lua_valid = Some(false);
                                errors.push(error.to_string());
                            }
                        }
                    }
                }
                Err(error) => {
                    errors.push(error.to_string());
                }
            }
        }
        failed |= !entry.valid || lua_valid == Some(false) || !errors.is_empty();
        reports.push(PluginValidationReport {
            package: entry.clone(),
            lua_checked: lua_was_checked,
            lua_valid,
            errors,
        });
    }
    if json {
        print_json(&reports)?;
    } else if reports.is_empty() {
        println!("No plugin packages are installed.");
    } else {
        for report in &reports {
            let valid =
                report.package.valid && report.lua_valid != Some(false) && report.errors.is_empty();
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

fn set_enabled(
    requested: &str,
    requested_path: Option<String>,
    enable: bool,
    json: bool,
) -> Result<()> {
    let prepared = (|| {
        let project_root = path::determine_project_root(requested_path)?;
        let project_lock = project_tools::acquire_project_write_lock(
            &project_root,
            if enable {
                "enable a plugin"
            } else {
                "disable a plugin"
            },
        )?;
        let config = load_config(Some(&project_root))?;
        let extension_locks = enable
            .then(|| {
                acquire_extension_store_read_locks(Some(&project_root), &config, "enable a plugin")
            })
            .transpose()?;
        let requested_spec = package_spec(requested)?;
        let mut enabled = config.plugins_enabled.clone();
        enabled.retain(|value| {
            package_spec(value)
                .map(|spec| spec.id != requested_spec.id)
                .unwrap_or(true)
        });
        if enable {
            let plugin = resolve_plugin_request(Some(&project_root), &config, requested)?;
            ensure_pandoc_compatible(std::slice::from_ref(&plugin.package), &config)?;
            let exact = format!("{}@={}", plugin.id, plugin.version);
            enabled.push(exact);
            let trusted = is_plugin_trusted(&plugin)?;
            if !json && !trusted {
                eprintln!(
                    "warning: {}@{} is enabled but remains inert until trusted on this machine",
                    plugin.id, plugin.version
                );
            }
        }
        let encoded = serde_json::to_string(&enabled)
            .map_err(|error| OmniDocError::Other(error.to_string()))?;
        Ok((project_root, encoded, project_lock, extension_locks))
    })();
    let (project_root, encoded, _project_lock, _extension_locks) = match prepared {
        Ok(prepared) => prepared,
        Err(error) => {
            if json {
                print_json_error(&error);
            }
            return Err(error);
        }
    };
    handle_project_config_set_locked("plugins.enabled".to_string(), encoded, &project_root, json)
}

fn set_trust(
    requested: &str,
    requested_project: Option<String>,
    trust: bool,
    json: bool,
) -> Result<()> {
    let (project_root, config) = catalog_context(requested_project)?;
    let _extension_locks = acquire_extension_store_read_locks(
        project_root.as_deref(),
        &config,
        if trust {
            "trust a plugin"
        } else {
            "revoke plugin trust"
        },
    )?;
    let plugin = resolve_plugin_request(project_root.as_deref(), &config, requested)?;
    let changed = if trust {
        trust_plugin(&plugin)?
    } else {
        revoke_plugin_trust(&plugin)?
    };
    let report = PluginTrustReport {
        schema_version: 1,
        id: plugin.id,
        version: plugin.version,
        digest: plugin.package.digest,
        trusted: trust,
        changed,
    };
    if json {
        print_json(&report)?;
    } else if trust {
        println!(
            "Trusted plugin {}@{} with digest {}.",
            report.id, report.version, report.digest
        );
    } else if changed {
        println!("Revoked trust for plugin {}@{}.", report.id, report.version);
    } else {
        println!("Plugin {}@{} was not trusted.", report.id, report.version);
    }
    Ok(())
}

fn run(
    requested: &str,
    command: &str,
    requested_project: Option<String>,
    arguments: &[String],
) -> Result<()> {
    let project_root = path::determine_project_context(requested_project)?;
    let config = load_config(Some(&project_root))?;
    let _extension_locks =
        acquire_extension_store_read_locks(Some(&project_root), &config, "run a plugin command")?;
    let plugin = resolve_plugin_request(Some(&project_root), &config, requested)?;
    run_plugin_command(&plugin, command, arguments, &project_root, &config)
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

fn matching_catalog_entry(
    project_root: Option<&Path>,
    config: &MergedConfig,
    plugin: &ResolvedPlugin,
) -> Result<PluginCatalogEntry> {
    let catalog = plugin_catalog(project_root, config)?;
    matching_entry(&catalog, plugin).cloned().ok_or_else(|| {
        OmniDocError::Other(format!(
            "resolved plugin '{}@{}' is missing from the catalog",
            plugin.id, plugin.version
        ))
    })
}

fn matching_entry<'a>(
    catalog: &'a [PluginCatalogEntry],
    plugin: &ResolvedPlugin,
) -> Option<&'a PluginCatalogEntry> {
    catalog.iter().find(|entry| {
        entry.id == plugin.id
            && entry.version == plugin.version
            && entry.digest.as_deref() == Some(plugin.package.digest.as_str())
    })
}

fn bundled_example_names(root: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut names = entries
        .filter_map(std::result::Result::ok)
        .filter(|entry| {
            entry.file_type().is_ok_and(|kind| kind.is_dir())
                && entry.path().join(PACKAGE_MANIFEST_FILE).is_file()
        })
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    names.sort();
    names
}

fn safe_example_key(value: &str) -> bool {
    !value.is_empty()
        && !value.contains(['/', '\\'])
        && Path::new(value)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
        && value.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '-' | '_')
        })
}

fn plugin_json_mode(command: &PluginSubcommand) -> bool {
    match command {
        PluginSubcommand::Install { json, .. }
        | PluginSubcommand::InstallExample { json, .. }
        | PluginSubcommand::Uninstall { json, .. }
        | PluginSubcommand::List { json, .. }
        | PluginSubcommand::Inspect { json, .. }
        | PluginSubcommand::Validate { json, .. }
        | PluginSubcommand::Enable { json, .. }
        | PluginSubcommand::Disable { json, .. }
        | PluginSubcommand::Trust { json, .. }
        | PluginSubcommand::Untrust { json, .. } => *json,
        PluginSubcommand::Run { .. } => false,
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
    use super::{bundled_example_names, safe_example_key, PACKAGE_MANIFEST_FILE};
    use std::fs;

    #[test]
    fn example_keys_cannot_escape_the_bundle_directory() {
        assert!(safe_example_key("quality-gate"));
        assert!(safe_example_key("word_count"));
        assert!(!safe_example_key("../quality-gate"));
        assert!(!safe_example_key("nested/example"));
        assert!(!safe_example_key(""));
    }

    #[test]
    fn example_catalog_ignores_directories_without_package_manifests() {
        let root = tempfile::tempdir().expect("example root");
        let valid = root.path().join("valid-example");
        let stale = root.path().join("removed-example");
        fs::create_dir_all(&valid).expect("valid example");
        fs::create_dir_all(&stale).expect("stale example");
        fs::write(valid.join(PACKAGE_MANIFEST_FILE), "manifest_version = 2\n")
            .expect("example manifest");

        assert_eq!(bundled_example_names(root.path()), vec!["valid-example"]);
    }
}
