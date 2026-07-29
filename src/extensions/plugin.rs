use super::package::{
    ensure_pandoc_compatible, normalized_output, package_records, package_spec,
    tracked_package_files, ExtensionResource, PackageInspection, PackageKind, PackageRecord,
    PackageScope, PackageSpec, ResolvedPackageIdentity, PACKAGE_MANIFEST_FILE,
};
use crate::config::MergedConfig;
use crate::error::{OmniDocError, Result};
use crate::utils::directories::config_local_dir;
use fs2::FileExt;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const TRUST_FILE_VERSION: u32 = 1;
const TRUST_FILE_NAME: &str = "omnidoc-plugin-trust.json";
const VALIDATION_SCRIPT_ENV: &str = "OMNIDOC_PLUGIN_VALIDATE_SCRIPT";

#[derive(Debug, Clone)]
pub struct ResolvedPlugin {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub root: PathBuf,
    pub filters: Vec<ResolvedPluginFilter>,
    pub commands: Vec<ResolvedPluginCommand>,
    pub package: ResolvedPackageIdentity,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResolvedPluginFilter {
    pub plugin_id: String,
    pub plugin_version: String,
    pub script: PathBuf,
    pub formats: Vec<String>,
    pub order: i32,
    pub dependency_key: Option<String>,
}

impl ResolvedPluginFilter {
    pub fn depfile_name(&self) -> Option<String> {
        self.dependency_key
            .as_deref()
            .map(|key| format!("plugin-{key}.d"))
    }

    pub fn depfile_metadata_key(&self) -> Option<String> {
        self.dependency_key
            .as_deref()
            .map(|key| format!("omnidoc-plugin-depfile-{key}"))
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ResolvedPluginCommand {
    pub name: String,
    pub script: PathBuf,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PluginCatalogEntry {
    pub manifest_path: String,
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub compatible_omnidoc: Option<String>,
    pub compatible_pandoc: Option<String>,
    pub source: String,
    pub scope: PackageScope,
    pub digest: Option<String>,
    pub filters: Vec<PluginCatalogFilter>,
    pub commands: Vec<PluginCatalogCommand>,
    pub enabled: bool,
    pub trusted: bool,
    pub valid: bool,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PluginCatalogFilter {
    pub script: String,
    pub formats: Vec<String>,
    pub order: i32,
    pub dependency_key: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PluginCatalogCommand {
    pub name: String,
    pub script: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PluginTrustFile {
    trust_version: u32,
    #[serde(default)]
    plugins: BTreeMap<String, PluginTrustEntry>,
}

impl Default for PluginTrustFile {
    fn default() -> Self {
        Self {
            trust_version: TRUST_FILE_VERSION,
            plugins: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PluginTrustEntry {
    id: String,
    version: String,
    digest: String,
    trusted_at_unix: u64,
}

struct PluginTrustLock {
    file: fs::File,
}

impl Drop for PluginTrustLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

pub fn plugin_catalog(
    project_root: Option<&Path>,
    config: &MergedConfig,
) -> Result<Vec<PluginCatalogEntry>> {
    let enabled_specs = enabled_plugin_specs(config)?;
    let trust = load_trust_file()?;
    let inspections = super::package::discover_packages(PackageKind::Plugin, project_root, config)?;
    let active_manifests = active_plugin_manifest_paths(&inspections, &enabled_specs);
    let mut entries = Vec::new();
    for inspection in inspections {
        let manifest = inspection.manifest.as_ref();
        let plugin = manifest.and_then(|manifest| manifest.plugin.as_ref());
        let enabled = active_manifests.contains(&inspection.manifest_path);
        let trusted = match (manifest, inspection.digest.as_deref(), inspection.valid) {
            (Some(manifest), Some(digest), true) => {
                trust
                    .plugins
                    .contains_key(&trust_key(&manifest.id, &manifest.version, digest))
            }
            _ => false,
        };
        entries.push(PluginCatalogEntry {
            manifest_path: inspection.manifest_path,
            id: manifest
                .map(|manifest| manifest.id.clone())
                .unwrap_or_else(|| "invalid-plugin".to_string()),
            name: manifest
                .and_then(|manifest| manifest.name.clone())
                .or_else(|| manifest.map(|manifest| manifest.id.clone()))
                .unwrap_or_else(|| "Invalid plugin".to_string()),
            version: manifest
                .map(|manifest| manifest.version.clone())
                .unwrap_or_else(|| "unknown".to_string()),
            description: manifest.and_then(|manifest| manifest.description.clone()),
            compatible_omnidoc: manifest.map(|manifest| manifest.compatible_omnidoc.clone()),
            compatible_pandoc: manifest.and_then(|manifest| manifest.compatible_pandoc.clone()),
            source: inspection.source,
            scope: inspection.scope,
            digest: inspection.digest,
            filters: plugin
                .map(|plugin| {
                    plugin
                        .filters
                        .iter()
                        .map(|filter| PluginCatalogFilter {
                            script: filter.script.clone(),
                            formats: filter
                                .formats
                                .iter()
                                .map(|format| normalized_output(format))
                                .collect(),
                            order: filter.order,
                            dependency_key: filter.dependency_key.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            commands: plugin
                .map(|plugin| {
                    plugin
                        .commands
                        .iter()
                        .map(|command| PluginCatalogCommand {
                            name: command.name.clone(),
                            script: command.script.clone(),
                            description: command.description.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            enabled,
            trusted,
            valid: inspection.valid,
            errors: inspection.errors,
        });
    }
    entries.sort_by(|left, right| {
        left.id
            .cmp(&right.id)
            .then_with(|| {
                match (
                    Version::parse(&left.version),
                    Version::parse(&right.version),
                ) {
                    (Ok(left), Ok(right)) => left.cmp(&right),
                    _ => left.version.cmp(&right.version),
                }
            })
            .then_with(|| right.scope.cmp(&left.scope))
    });
    Ok(entries)
}

fn enabled_plugin_specs(config: &MergedConfig) -> Result<Vec<PackageSpec>> {
    let mut specs = Vec::new();
    let mut ids = BTreeSet::new();
    for requested in &config.plugins_enabled {
        let spec = package_spec(requested)?;
        if spec.exact_version().is_none() {
            return Err(OmniDocError::Config(format!(
                "enabled plugin '{}' must pin one exact version, for example '{}@=1.2.3'",
                requested, spec.id
            )));
        }
        if !ids.insert(spec.id.clone()) {
            return Err(OmniDocError::Config(format!(
                "plugin '{}' is enabled more than once",
                spec.id
            )));
        }
        specs.push(spec);
    }
    Ok(specs)
}

fn active_plugin_manifest_paths(
    inspections: &[PackageInspection],
    specs: &[PackageSpec],
) -> BTreeSet<String> {
    let mut active = BTreeSet::new();
    for spec in specs {
        let selected = inspections
            .iter()
            .filter(|inspection| inspection.valid)
            .filter_map(|inspection| {
                let manifest = inspection.manifest.as_ref()?;
                if manifest.id != spec.id || !spec.matches_version(&manifest.version) {
                    return None;
                }
                let version = Version::parse(&manifest.version).ok()?;
                Some((
                    inspection.scope.priority(),
                    version,
                    inspection.manifest_path.as_str(),
                ))
            })
            .max_by(|left, right| {
                left.0
                    .cmp(&right.0)
                    .then_with(|| left.1.cmp(&right.1))
                    .then_with(|| left.2.cmp(right.2))
            });
        if let Some((_, _, manifest_path)) = selected {
            active.insert(manifest_path.to_string());
        }
    }
    active
}

pub fn resolve_plugin_request(
    project_root: Option<&Path>,
    config: &MergedConfig,
    request: &str,
) -> Result<ResolvedPlugin> {
    let spec = package_spec(request)?;
    let records = package_records(PackageKind::Plugin, project_root, config)?;
    select_plugin_record(records, &spec)
}

pub(crate) fn resolve_plugin_manifest(
    project_root: Option<&Path>,
    config: &MergedConfig,
    manifest_path: &Path,
) -> Result<ResolvedPlugin> {
    let record = package_records(PackageKind::Plugin, project_root, config)?
        .into_iter()
        .find(|record| record.root.join(PACKAGE_MANIFEST_FILE) == manifest_path)
        .ok_or_else(|| {
            OmniDocError::Config(format!(
                "plugin package at '{}' is invalid or no longer installed",
                manifest_path.display()
            ))
        })?;
    record_to_plugin(record)
}

pub fn enabled_plugins(project_root: &Path, config: &MergedConfig) -> Result<Vec<ResolvedPlugin>> {
    let specs = enabled_plugin_specs(config)?;
    if specs.is_empty() {
        return Ok(Vec::new());
    }
    let records = package_records(PackageKind::Plugin, Some(project_root), config)?;
    let mut plugins = Vec::new();
    for spec in specs {
        let plugin = select_plugin_record(records.clone(), &spec)?;
        if !is_plugin_trusted(&plugin)? {
            return Err(OmniDocError::Project(format!(
                "plugin '{}@{}' is enabled but not trusted on this machine; run `omnidoc plugin trust {}@={}`",
                plugin.id, plugin.version, plugin.id, plugin.version
            )));
        }
        plugins.push(plugin);
    }
    let mut dependency_keys = BTreeMap::new();
    for plugin in &plugins {
        for filter in &plugin.filters {
            let Some(key) = filter.dependency_key.as_deref() else {
                continue;
            };
            let origin = format!(
                "{}@{}:{}",
                plugin.id,
                plugin.version,
                filter.script.display()
            );
            if let Some(existing) = dependency_keys.insert(key.to_string(), origin.clone()) {
                return Err(OmniDocError::Config(format!(
                    "enabled plugin filters '{}' and '{}' declare the same dependency_key '{}'",
                    existing, origin, key
                )));
            }
        }
    }
    let packages = plugins
        .iter()
        .map(|plugin| plugin.package.clone())
        .collect::<Vec<_>>();
    ensure_pandoc_compatible(&packages, config)?;
    plugins.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(plugins)
}

fn select_plugin_record(records: Vec<PackageRecord>, spec: &PackageSpec) -> Result<ResolvedPlugin> {
    let mut candidates = records
        .into_iter()
        .filter(|record| record.manifest.id == spec.id)
        .filter_map(|record| {
            let version = Version::parse(&record.manifest.version).ok()?;
            spec.matches_version(&record.manifest.version)
                .then_some((version, record))
        })
        .collect::<Vec<_>>();
    let priority = candidates
        .iter()
        .map(|(_, record)| record.scope.priority())
        .max()
        .ok_or_else(|| {
            OmniDocError::Config(format!(
                "plugin '{}' is not installed or has no version satisfying {}",
                spec.id,
                spec.raw_requirement.as_deref().unwrap_or("*")
            ))
        })?;
    candidates.retain(|(_, record)| record.scope.priority() == priority);
    let (_, record) = candidates
        .into_iter()
        .max_by(|left, right| left.0.cmp(&right.0))
        .ok_or_else(|| {
            OmniDocError::Config(format!("plugin '{}' could not be resolved", spec.id))
        })?;
    record_to_plugin(record)
}

fn record_to_plugin(record: PackageRecord) -> Result<ResolvedPlugin> {
    let definition = record.manifest.plugin.clone().ok_or_else(|| {
        OmniDocError::Other(format!(
            "plugin '{}' has no [plugin] section",
            record.manifest.id
        ))
    })?;
    let compatible_pandoc = record.manifest.compatible_pandoc.clone();
    let tracked_files = tracked_package_files(&record.root)?;
    let filters = definition
        .filters
        .into_iter()
        .map(|filter| ResolvedPluginFilter {
            plugin_id: record.manifest.id.clone(),
            plugin_version: record.manifest.version.clone(),
            script: record.root.join(filter.script),
            formats: filter
                .formats
                .iter()
                .map(|format| normalized_output(format))
                .collect(),
            order: filter.order,
            dependency_key: filter.dependency_key,
        })
        .collect();
    let commands = definition
        .commands
        .into_iter()
        .map(|command| ResolvedPluginCommand {
            name: command.name,
            script: record.root.join(command.script),
            description: command.description,
        })
        .collect();
    Ok(ResolvedPlugin {
        id: record.manifest.id.clone(),
        name: record
            .manifest
            .name
            .clone()
            .unwrap_or_else(|| record.manifest.id.clone()),
        version: record.manifest.version.clone(),
        description: record.manifest.description.clone(),
        root: record.root.clone(),
        filters,
        commands,
        package: ResolvedPackageIdentity {
            kind: PackageKind::Plugin,
            scope: record.scope,
            id: record.manifest.id,
            version: record.manifest.version,
            source: record.source,
            digest: record.digest,
            root: record.root,
            tracked_files,
            compatible_pandoc,
        },
    })
}

pub fn plugin_filters_for_output(
    project_root: &Path,
    config: &MergedConfig,
    output: &str,
) -> Result<Vec<ResolvedPluginFilter>> {
    let output = normalized_output(output);
    let mut filters = enabled_plugins(project_root, config)?
        .into_iter()
        .flat_map(|plugin| plugin.filters)
        .filter(|filter| filter.formats.is_empty() || filter.formats.contains(&output))
        .collect::<Vec<_>>();
    filters.sort_by(|left, right| {
        left.order
            .cmp(&right.order)
            .then_with(|| left.plugin_id.cmp(&right.plugin_id))
            .then_with(|| left.script.cmp(&right.script))
    });
    Ok(filters)
}

pub fn enabled_plugin_resources(
    project_root: &Path,
    config: &MergedConfig,
) -> Result<Vec<ExtensionResource>> {
    let mut resources = Vec::new();
    for plugin in enabled_plugins(project_root, config)? {
        for path in &plugin.package.tracked_files {
            let relative = path
                .strip_prefix(&plugin.package.root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            resources.push(ExtensionResource {
                logical_name: format!("plugin-package:{}:{}", plugin.id, relative),
                resolved_from: extension_source_label(plugin.package.scope),
                path: path.clone(),
            });
        }
    }
    Ok(resources)
}

fn extension_source_label(scope: PackageScope) -> String {
    match scope {
        PackageScope::Builtin => "omnidoc-libs".to_string(),
        PackageScope::Project => "extension-project".to_string(),
        PackageScope::User => "extension-user".to_string(),
    }
}

pub fn is_plugin_trusted(plugin: &ResolvedPlugin) -> Result<bool> {
    is_trust_key_present(&trust_key(
        &plugin.id,
        &plugin.version,
        &plugin.package.digest,
    ))
}

pub fn trust_plugin(plugin: &ResolvedPlugin) -> Result<bool> {
    let _lock = acquire_trust_lock()?;
    let mut trust = load_trust_file()?;
    let key = trust_key(&plugin.id, &plugin.version, &plugin.package.digest);
    let changed = trust
        .plugins
        .insert(
            key,
            PluginTrustEntry {
                id: plugin.id.clone(),
                version: plugin.version.clone(),
                digest: plugin.package.digest.clone(),
                trusted_at_unix: current_timestamp_unix(),
            },
        )
        .is_none();
    if changed {
        write_trust_file(&trust)?;
    }
    Ok(changed)
}

pub fn revoke_plugin_trust(plugin: &ResolvedPlugin) -> Result<bool> {
    let _lock = acquire_trust_lock()?;
    let mut trust = load_trust_file()?;
    let removed = trust
        .plugins
        .remove(&trust_key(
            &plugin.id,
            &plugin.version,
            &plugin.package.digest,
        ))
        .is_some();
    if removed {
        write_trust_file(&trust)?;
    }
    Ok(removed)
}

fn is_trust_key_present(key: &str) -> Result<bool> {
    Ok(load_trust_file()?.plugins.contains_key(key))
}

fn trust_key(id: &str, version: &str, digest: &str) -> String {
    format!("{id}@{version}#{digest}")
}

pub(crate) fn plugin_trust_path() -> Result<PathBuf> {
    if let Some(path) =
        std::env::var_os("OMNIDOC_PLUGIN_TRUST_FILE").filter(|path| !path.is_empty())
    {
        return Ok(PathBuf::from(path));
    }
    config_local_dir()
        .map(|path| path.join(TRUST_FILE_NAME))
        .ok_or_else(|| OmniDocError::Config("Local config directory not found".to_string()))
}

fn acquire_trust_lock() -> Result<PluginTrustLock> {
    let trust_path = plugin_trust_path()?;
    if let Some(parent) = trust_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let mut lock_name = trust_path.as_os_str().to_os_string();
    lock_name.push(".lock");
    let lock_path = PathBuf::from(lock_name);
    if lock_path
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err(OmniDocError::Other(format!(
            "plugin trust lock must not be a symbolic link: {}",
            lock_path.display()
        )));
    }
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)?;
    FileExt::try_lock_exclusive(&file).map_err(|error| {
        OmniDocError::Other(format!(
            "cannot update plugin trust: another OmniDoc process holds {} ({error})",
            lock_path.display()
        ))
    })?;
    Ok(PluginTrustLock { file })
}

fn load_trust_file() -> Result<PluginTrustFile> {
    let path = plugin_trust_path()?;
    if !path.is_file() {
        return Ok(PluginTrustFile::default());
    }
    let content = fs::read_to_string(&path)?;
    let trust: PluginTrustFile = serde_json::from_str(&content).map_err(|error| {
        OmniDocError::Config(format!(
            "failed to parse plugin trust file {}: {error}",
            path.display()
        ))
    })?;
    if trust.trust_version != TRUST_FILE_VERSION {
        return Err(OmniDocError::Config(format!(
            "unsupported plugin trust file version {}",
            trust.trust_version
        )));
    }
    Ok(trust)
}

fn write_trust_file(trust: &PluginTrustFile) -> Result<()> {
    let path = plugin_trust_path()?;
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let content =
        serde_json::to_vec_pretty(trust).map_err(|error| OmniDocError::Other(error.to_string()))?;
    crate::utils::fs::atomic_write(path, content)
}

pub fn validate_plugin_lua(plugin: &ResolvedPlugin, config: &MergedConfig) -> Result<()> {
    ensure_pandoc_compatible(std::slice::from_ref(&plugin.package), config)?;
    let pandoc = configured_pandoc(config);
    for script in plugin
        .filters
        .iter()
        .map(|filter| &filter.script)
        .chain(plugin.commands.iter().map(|command| &command.script))
    {
        let output = Command::new(&pandoc)
            .args([
                "lua",
                "-e",
                "local script = assert(os.getenv('OMNIDOC_PLUGIN_VALIDATE_SCRIPT')); assert(loadfile(script))",
            ])
            .env(VALIDATION_SCRIPT_ENV, script)
            .output()
            .map_err(|error| {
                OmniDocError::Other(format!(
                    "failed to validate Lua script {} with Pandoc: {error}",
                    script.display()
                ))
            })?;
        if !output.status.success() {
            return Err(OmniDocError::Other(format!(
                "invalid Pandoc Lua script {}:\n{}{}",
                script.display(),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )));
        }
    }
    Ok(())
}

pub fn run_plugin_command(
    plugin: &ResolvedPlugin,
    command_name: &str,
    arguments: &[String],
    project_root: &Path,
    config: &MergedConfig,
) -> Result<()> {
    if !is_plugin_trusted(plugin)? {
        return Err(OmniDocError::Project(format!(
            "plugin '{}@{}' is not trusted; run `omnidoc plugin trust {}@={}` first",
            plugin.id, plugin.version, plugin.id, plugin.version
        )));
    }
    ensure_pandoc_compatible(std::slice::from_ref(&plugin.package), config)?;
    let command = plugin
        .commands
        .iter()
        .find(|command| command.name == command_name)
        .ok_or_else(|| {
            let available = plugin
                .commands
                .iter()
                .map(|command| command.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            OmniDocError::Project(format!(
                "plugin '{}' has no command '{}'; available commands: {}",
                plugin.id,
                command_name,
                if available.is_empty() {
                    "none"
                } else {
                    &available
                }
            ))
        })?;
    let status = Command::new(configured_pandoc(config))
        .arg("lua")
        .arg(&command.script)
        .args(arguments)
        .current_dir(project_root)
        .env("OMNIDOC_PROJECT_DIR", project_root)
        .env("OMNIDOC_PLUGIN_DIR", &plugin.root)
        .env("OMNIDOC_PLUGIN_ID", &plugin.id)
        .env("OMNIDOC_PLUGIN_VERSION", &plugin.version)
        .status()
        .map_err(|error| {
            OmniDocError::CommandExecution(format!(
                "failed to run Pandoc Lua command '{}:{}': {error}",
                plugin.id, command_name
            ))
        })?;
    if !status.success() {
        return Err(OmniDocError::CommandNonZeroExit {
            code: status.code(),
            command: format!("pandoc lua {}", command.script.display()),
        });
    }
    Ok(())
}

fn configured_pandoc(config: &MergedConfig) -> String {
    config
        .tool_paths
        .get("pandoc")
        .and_then(|value| value.clone())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "pandoc".to_string())
}

fn current_timestamp_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{
        enabled_plugin_resources, enabled_plugins, is_plugin_trusted, plugin_catalog,
        plugin_filters_for_output, resolve_plugin_manifest, resolve_plugin_request,
        run_plugin_command, trust_key, trust_plugin, validate_plugin_lua,
    };
    use crate::config::MergedConfig;
    use crate::extensions::package::PackageScope;
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, OnceLock};

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    struct EnvGuard {
        key: &'static str,
        previous: Option<OsString>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &Path) -> Self {
            let previous = std::env::var_os(key);
            std::env::set_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(previous) = self.previous.take() {
                std::env::set_var(self.key, previous);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }

    fn package_path(store: &Path, id: &str) -> PathBuf {
        id.split('/')
            .fold(store.join("plugins"), |path, segment| path.join(segment))
            .join("1.0.0")
    }

    fn project_package_path(project: &Path, id: &str) -> PathBuf {
        id.split('/')
            .fold(
                project.join(".omnidoc/extensions/plugins"),
                |path, segment| path.join(segment),
            )
            .join("1.0.0")
    }

    fn write_filter_package(
        root: &Path,
        id: &str,
        order: i32,
        formats: &[&str],
        dependency_key: Option<&str>,
        body: &str,
    ) {
        fs::create_dir_all(root.join("filters")).expect("filter directory");
        fs::write(root.join("filters/main.lua"), body).expect("filter body");
        let formats = formats
            .iter()
            .map(|format| format!("\"{format}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let dependency_key = dependency_key
            .map(|key| format!("dependency_key = \"{key}\"\n"))
            .unwrap_or_default();
        fs::write(
            root.join(super::super::package::PACKAGE_MANIFEST_FILE),
            format!(
                r#"manifest_version = 2
kind = "plugin"
id = "{id}"
version = "1.0.0"
compatible_omnidoc = ">=1.8,<2"
compatible_pandoc = "*"

[plugin]
api_version = 1

[[plugin.filters]]
script = "filters/main.lua"
formats = [{formats}]
order = {order}
{dependency_key}
"#
            ),
        )
        .expect("plugin manifest");
    }

    fn write_command_package(root: &Path) {
        fs::create_dir_all(root.join("commands")).expect("command directory");
        fs::write(root.join("commands/tool.lua"), "return true\n").expect("command body");
        fs::write(
            root.join(super::super::package::PACKAGE_MANIFEST_FILE),
            r#"manifest_version = 2
kind = "plugin"
id = "acme/tools"
version = "1.0.0"
compatible_omnidoc = ">=1.8,<2"
compatible_pandoc = "*"

[plugin]
api_version = 1

[[plugin.commands]]
name = "echo"
script = "commands/tool.lua"
"#,
        )
        .expect("command manifest");
    }

    #[test]
    fn trust_keys_bind_identity_version_and_digest() {
        assert_eq!(
            trust_key("acme/check", "1.2.0", "sha256:abc"),
            "acme/check@1.2.0#sha256:abc"
        );
    }

    #[test]
    fn catalog_reports_a_corrupted_trust_file() {
        let _environment_lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let workspace = tempfile::tempdir().expect("workspace");
        let user_store = workspace.path().join("user-store");
        let trust_path = workspace.path().join("trust.json");
        let _trust = EnvGuard::set("OMNIDOC_PLUGIN_TRUST_FILE", &trust_path);
        write_filter_package(
            &package_path(&user_store, "acme/check"),
            "acme/check",
            100,
            &["html"],
            None,
            "return {}\n",
        );
        fs::write(&trust_path, "{\n").expect("corrupted trust file");
        let config = MergedConfig {
            extension_path: Some(user_store.to_string_lossy().to_string()),
            ..Default::default()
        };

        let error = plugin_catalog(None, &config).expect_err("trust corruption must be visible");
        assert!(error
            .to_string()
            .contains("failed to parse plugin trust file"));
    }

    #[test]
    fn plugins_are_inert_until_enabled_and_trusted_and_filters_are_ordered() {
        let _environment_lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let workspace = tempfile::tempdir().expect("workspace");
        let project = workspace.path().join("project");
        let user_store = workspace.path().join("user-store");
        fs::create_dir_all(&project).expect("project");
        let _trust = EnvGuard::set(
            "OMNIDOC_PLUGIN_TRUST_FILE",
            &workspace.path().join("trust.json"),
        );

        write_filter_package(
            &project_package_path(&project, "acme/zeta"),
            "acme/zeta",
            200,
            &["html"],
            None,
            "return {}\n",
        );
        write_filter_package(
            &package_path(&user_store, "acme/alpha"),
            "acme/alpha",
            100,
            &["html", "pdf"],
            Some("acme-alpha-inputs"),
            "return {}\n",
        );
        write_filter_package(
            &package_path(&user_store, "acme/pdf-only"),
            "acme/pdf-only",
            50,
            &["pdf"],
            None,
            "return {}\n",
        );
        let mut config = MergedConfig {
            extension_path: Some(user_store.to_string_lossy().to_string()),
            ..Default::default()
        };

        let catalog = plugin_catalog(Some(&project), &config).expect("plugin catalog");
        assert_eq!(catalog.len(), 3);
        assert!(catalog.iter().all(|plugin| !plugin.enabled));
        assert!(enabled_plugins(&project, &config)
            .expect("no enabled plugins")
            .is_empty());

        for request in [
            "acme/zeta@=1.0.0",
            "acme/alpha@=1.0.0",
            "acme/pdf-only@=1.0.0",
        ] {
            let plugin =
                resolve_plugin_request(Some(&project), &config, request).expect("resolve plugin");
            trust_plugin(&plugin).expect("trust plugin");
        }
        config.plugins_enabled = vec![
            "acme/zeta@=1.0.0".to_string(),
            "acme/pdf-only@=1.0.0".to_string(),
            "acme/alpha@=1.0.0".to_string(),
        ];
        let filters =
            plugin_filters_for_output(&project, &config, "html").expect("ordered HTML filters");
        assert_eq!(
            filters
                .iter()
                .map(|filter| filter.plugin_id.as_str())
                .collect::<Vec<_>>(),
            ["acme/alpha", "acme/zeta"]
        );
        let alpha = filters
            .iter()
            .find(|filter| filter.plugin_id == "acme/alpha")
            .expect("alpha filter");
        assert_eq!(
            alpha.depfile_name().as_deref(),
            Some("plugin-acme-alpha-inputs.d")
        );
        assert_eq!(
            alpha.depfile_metadata_key().as_deref(),
            Some("omnidoc-plugin-depfile-acme-alpha-inputs")
        );
        let resources =
            enabled_plugin_resources(&project, &config).expect("enabled plugin resources");
        assert!(resources.iter().any(|resource| {
            resource
                .logical_name
                .starts_with("plugin-package:acme/zeta:")
                && resource.resolved_from == "extension-project"
        }));
        assert!(resources.iter().any(|resource| {
            resource
                .logical_name
                .starts_with("plugin-package:acme/alpha:")
                && resource.resolved_from == "extension-user"
        }));

        let before = resolve_plugin_request(Some(&project), &config, "acme/zeta@=1.0.0")
            .expect("trusted plugin");
        assert!(is_plugin_trusted(&before).expect("trust state"));
        fs::write(
            project_package_path(&project, "acme/zeta").join("filters/main.lua"),
            "return { Pandoc = function(doc) return doc end }\n",
        )
        .expect("mutate installed payload");
        let changed = resolve_plugin_request(Some(&project), &config, "acme/zeta@=1.0.0")
            .expect("changed plugin");
        assert_ne!(before.package.digest, changed.package.digest);
        assert!(!is_plugin_trusted(&changed).expect("changed trust state"));
        assert!(enabled_plugins(&project, &config)
            .expect_err("changed payload must invalidate enabled trust")
            .to_string()
            .contains("not trusted"));
    }

    #[test]
    fn enabled_plugins_require_exact_versions_and_unique_dependency_keys() {
        let _environment_lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let workspace = tempfile::tempdir().expect("workspace");
        let project = workspace.path().join("project");
        let user_store = workspace.path().join("user-store");
        fs::create_dir_all(&project).expect("project");
        let _trust = EnvGuard::set(
            "OMNIDOC_PLUGIN_TRUST_FILE",
            &workspace.path().join("trust.json"),
        );
        write_filter_package(
            &package_path(&user_store, "acme/first"),
            "acme/first",
            100,
            &["html"],
            Some("acme-shared-inputs"),
            "return {}\n",
        );
        write_filter_package(
            &package_path(&user_store, "acme/second"),
            "acme/second",
            200,
            &["html"],
            Some("acme-shared-inputs"),
            "return {}\n",
        );
        let mut config = MergedConfig {
            extension_path: Some(user_store.to_string_lossy().to_string()),
            plugins_enabled: vec!["acme/first@^1".to_string()],
            ..Default::default()
        };
        let error = enabled_plugins(&project, &config).expect_err("range enable must fail");
        assert!(error.to_string().contains("must pin one exact version"));

        for request in ["acme/first@=1.0.0", "acme/second@=1.0.0"] {
            let plugin =
                resolve_plugin_request(Some(&project), &config, request).expect("resolve plugin");
            trust_plugin(&plugin).expect("trust plugin");
        }
        config.plugins_enabled = vec![
            "acme/first@=1.0.0".to_string(),
            "acme/second@=1.0.0".to_string(),
        ];
        let error = enabled_plugins(&project, &config).expect_err("duplicate key must fail");
        assert!(error.to_string().contains("same dependency_key"));
    }

    #[test]
    fn exact_plugin_resolution_distinguishes_build_metadata() {
        let workspace = tempfile::tempdir().expect("workspace");
        let store = workspace.path().join("user-store");
        for version in ["1.0.0+linux", "1.0.0+windows"] {
            let root = store.join("plugins/acme/platform").join(version);
            write_filter_package(&root, "acme/platform", 500, &["html"], None, "return {}\n");
            let manifest_path = root.join(super::super::package::PACKAGE_MANIFEST_FILE);
            let manifest = fs::read_to_string(&manifest_path)
                .expect("manifest")
                .replace("version = \"1.0.0\"", &format!("version = \"{version}\""));
            fs::write(manifest_path, manifest).expect("versioned manifest");
        }
        let config = MergedConfig {
            extension_path: Some(store.to_string_lossy().to_string()),
            ..Default::default()
        };

        let linux = resolve_plugin_request(None, &config, "acme/platform@=1.0.0+linux")
            .expect("Linux package");
        let windows = resolve_plugin_request(None, &config, "acme/platform@=1.0.0+windows")
            .expect("Windows package");

        assert_eq!(linux.version, "1.0.0+linux");
        assert_eq!(windows.version, "1.0.0+windows");
        assert_ne!(linux.root, windows.root);
    }

    #[test]
    fn catalog_marks_only_the_resolved_exact_payload_as_enabled() {
        let _environment_lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let workspace = tempfile::tempdir().expect("workspace");
        let project = workspace.path().join("project");
        let store = workspace.path().join("user-store");
        fs::create_dir_all(&project).expect("project");
        let _trust = EnvGuard::set(
            "OMNIDOC_PLUGIN_TRUST_FILE",
            &workspace.path().join("trust.json"),
        );

        let package_root =
            |base: &Path, version: &str| base.join("plugins/acme/platform").join(version);
        for version in ["1.0.0+linux", "1.0.0+windows"] {
            let root = package_root(&store, version);
            write_filter_package(
                &root,
                "acme/platform",
                500,
                &["html"],
                None,
                "return { source = 'user' }\n",
            );
            let manifest_path = root.join(super::super::package::PACKAGE_MANIFEST_FILE);
            let manifest = fs::read_to_string(&manifest_path)
                .expect("manifest")
                .replace("version = \"1.0.0\"", &format!("version = \"{version}\""));
            fs::write(manifest_path, manifest).expect("versioned manifest");
        }

        let project_root = project
            .join(".omnidoc/extensions/plugins/acme/platform")
            .join("1.0.0+linux");
        write_filter_package(
            &project_root,
            "acme/platform",
            500,
            &["html"],
            None,
            "return { source = 'project' }\n",
        );
        let manifest_path = project_root.join(super::super::package::PACKAGE_MANIFEST_FILE);
        let manifest = fs::read_to_string(&manifest_path)
            .expect("project manifest")
            .replace("version = \"1.0.0\"", "version = \"1.0.0+linux\"");
        fs::write(manifest_path, manifest).expect("project versioned manifest");

        let config = MergedConfig {
            extension_path: Some(store.to_string_lossy().to_string()),
            plugins_enabled: vec!["acme/platform@=1.0.0+linux".to_string()],
            ..Default::default()
        };
        let catalog = plugin_catalog(Some(&project), &config).expect("plugin catalog");
        let platform = catalog
            .iter()
            .filter(|entry| entry.id == "acme/platform")
            .collect::<Vec<_>>();

        assert_eq!(platform.len(), 3);
        let enabled = platform
            .iter()
            .filter(|entry| entry.enabled)
            .collect::<Vec<_>>();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].scope, PackageScope::Project);
        assert_eq!(enabled[0].version, "1.0.0+linux");
    }

    #[test]
    fn manifest_resolution_can_validate_shadowed_plugin_packages() {
        let workspace = tempfile::tempdir().expect("workspace");
        let project = workspace.path().join("project");
        let user_store = workspace.path().join("user-store");
        fs::create_dir_all(&project).expect("project");
        let user_package = package_path(&user_store, "acme/shared");
        let project_package = project_package_path(&project, "acme/shared");
        write_filter_package(
            &user_package,
            "acme/shared",
            100,
            &["html"],
            None,
            "return { user = true }\n",
        );
        write_filter_package(
            &project_package,
            "acme/shared",
            100,
            &["html"],
            None,
            "return { project = true }\n",
        );
        let config = MergedConfig {
            extension_path: Some(user_store.to_string_lossy().to_string()),
            ..Default::default()
        };

        let selected = resolve_plugin_request(Some(&project), &config, "acme/shared@=1.0.0")
            .expect("project package wins normal resolution");
        assert_eq!(selected.root, project_package);

        let shadowed = resolve_plugin_manifest(
            Some(&project),
            &config,
            &user_package.join(super::super::package::PACKAGE_MANIFEST_FILE),
        )
        .expect("resolve shadowed user package by manifest");
        assert_eq!(shadowed.root, user_package);
        assert_ne!(shadowed.package.digest, selected.package.digest);
    }

    #[cfg(unix)]
    #[test]
    fn lua_validation_compiles_the_script_without_passing_it_for_execution() {
        use std::os::unix::fs::PermissionsExt;

        let workspace = tempfile::tempdir().expect("workspace");
        let project = workspace.path().join("project");
        let user_store = workspace.path().join("user-store");
        fs::create_dir_all(&project).expect("project");
        write_filter_package(
            &package_path(&user_store, "acme/check"),
            "acme/check",
            100,
            &["html"],
            None,
            "io.stderr:write('must not execute during validation\\n')\n",
        );
        let arguments = workspace.path().join("arguments.txt");
        let validation_script = workspace.path().join("validation-script.txt");
        let fake_pandoc = workspace.path().join("pandoc");
        fs::write(
            &fake_pandoc,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\nprintf '%s' \"$OMNIDOC_PLUGIN_VALIDATE_SCRIPT\" > '{}'\n",
                arguments.display(),
                validation_script.display()
            ),
        )
        .expect("fake pandoc");
        let mut permissions = fs::metadata(&fake_pandoc).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_pandoc, permissions).expect("permissions");
        let mut config = MergedConfig {
            extension_path: Some(user_store.to_string_lossy().to_string()),
            ..Default::default()
        };
        config.tool_paths.insert(
            "pandoc".to_string(),
            Some(fake_pandoc.to_string_lossy().to_string()),
        );
        let plugin = resolve_plugin_request(Some(&project), &config, "acme/check@=1.0.0")
            .expect("resolve plugin");

        validate_plugin_lua(&plugin, &config).expect("validate Lua syntax");

        let captured = fs::read_to_string(arguments).expect("captured arguments");
        let lines = captured.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "lua");
        assert_eq!(lines[1], "-e");
        assert!(lines[2].contains("loadfile(script)"));
        assert!(!lines
            .iter()
            .any(|argument| Path::new(argument) == plugin.filters[0].script));
        assert_eq!(
            PathBuf::from(fs::read_to_string(validation_script).expect("captured script path")),
            plugin.filters[0].script
        );
    }

    #[cfg(unix)]
    #[test]
    fn explicit_commands_are_invoked_only_through_pandoc_lua() {
        use std::os::unix::fs::PermissionsExt;

        let _environment_lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let workspace = tempfile::tempdir().expect("workspace");
        let project = workspace.path().join("project");
        let user_store = workspace.path().join("user-store");
        fs::create_dir_all(&project).expect("project");
        let _trust = EnvGuard::set(
            "OMNIDOC_PLUGIN_TRUST_FILE",
            &workspace.path().join("trust.json"),
        );
        write_command_package(&package_path(&user_store, "acme/tools"));
        let capture = workspace.path().join("arguments.txt");
        let fake_pandoc = workspace.path().join("pandoc");
        fs::write(
            &fake_pandoc,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\n",
                capture.display()
            ),
        )
        .expect("fake pandoc");
        let mut permissions = fs::metadata(&fake_pandoc).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_pandoc, permissions).expect("permissions");
        let mut config = MergedConfig {
            extension_path: Some(user_store.to_string_lossy().to_string()),
            ..Default::default()
        };
        config.tool_paths.insert(
            "pandoc".to_string(),
            Some(fake_pandoc.to_string_lossy().to_string()),
        );
        let plugin = resolve_plugin_request(Some(&project), &config, "acme/tools@=1.0.0")
            .expect("command plugin");
        trust_plugin(&plugin).expect("trust command plugin");

        run_plugin_command(
            &plugin,
            "echo",
            &["first".to_string(), "second".to_string()],
            &project,
            &config,
        )
        .expect("run explicit command");
        let arguments = fs::read_to_string(capture).expect("captured arguments");
        let lines = arguments.lines().collect::<Vec<_>>();
        assert_eq!(lines[0], "lua");
        assert_eq!(Path::new(lines[1]), plugin.commands[0].script);
        assert_eq!(&lines[2..], ["first", "second"]);
    }
}
