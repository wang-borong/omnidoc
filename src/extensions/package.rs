use crate::config::MergedConfig;
use crate::error::{OmniDocError, Result};
use crate::utils::directories::data_local_dir;
use flate2::read::GzDecoder;
use fs2::FileExt;
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tar::Archive;
use walkdir::WalkDir;
use zip::ZipArchive;

pub const PACKAGE_MANIFEST_FILE: &str = "omnidoc-package.toml";
pub(super) const INSTALL_RECEIPT_FILE: &str = ".omnidoc-install.json";
const PACKAGE_MANIFEST_VERSION: u32 = 2;
const THEME_API_VERSION: u32 = 1;
const PLUGIN_API_VERSION: u32 = 1;
const MAX_ARCHIVE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_EXTRACTED_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_PACKAGE_ENTRIES: usize = 100_000;
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);
const PACKAGE_DIGEST_DOMAIN: &[u8] = b"omnidoc-package-digest-v3\0";
const STORE_LOCK_FILE: &str = ".omnidoc-store.lock";
const TRANSACTION_RECORD_FILE: &str = "transaction.json";
const TRANSACTION_RECORD_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum PackageKind {
    Theme,
    Plugin,
}

impl PackageKind {
    pub fn directory(self) -> &'static str {
        match self {
            Self::Theme => "themes",
            Self::Plugin => "plugins",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Theme => "theme",
            Self::Plugin => "plugin",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum PackageScope {
    Builtin,
    User,
    Project,
}

impl PackageScope {
    pub(super) fn priority(self) -> u8 {
        match self {
            Self::Builtin => 1,
            Self::User => 2,
            Self::Project => 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageManifest {
    pub manifest_version: u32,
    pub kind: PackageKind,
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    pub compatible_omnidoc: String,
    #[serde(default)]
    pub compatible_pandoc: Option<String>,
    #[serde(default)]
    pub checksum_file: Option<String>,
    #[serde(default)]
    pub theme: Option<ThemePackage>,
    #[serde(default)]
    pub plugin: Option<PluginPackage>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemePackage {
    #[serde(default = "default_theme_api_version")]
    pub api_version: u32,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub recommended_for: Vec<String>,
    #[serde(default)]
    pub compatibility: Option<String>,
    #[serde(default)]
    pub extends: Option<String>,
    #[serde(default)]
    pub outputs: Option<Vec<String>>,
    #[serde(default)]
    pub resources: ThemePackageResources,
    #[serde(default)]
    pub requirements: ThemePackageRequirements,
    #[serde(default)]
    pub metadata: ThemePackageMetadata,
    #[serde(default)]
    pub tokens: ThemeTokens,
}

fn default_theme_api_version() -> u32 {
    THEME_API_VERSION
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemePackageResources {
    #[serde(default)]
    pub html_css: Vec<String>,
    #[serde(default)]
    pub epub_css: Vec<String>,
    #[serde(default)]
    pub latex_packages: Vec<String>,
    #[serde(default)]
    pub latex_headers: Vec<String>,
    #[serde(default)]
    pub html_template: Option<String>,
    #[serde(default)]
    pub epub_template: Option<String>,
    #[serde(default)]
    pub latex_template: Option<String>,
    #[serde(default)]
    pub docx_reference_doc: Option<String>,
    #[serde(default)]
    pub pptx_reference_doc: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemePackageRequirements {
    #[serde(default)]
    pub fonts: Vec<String>,
    #[serde(default)]
    pub system_latex_packages: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemePackageMetadata {
    #[serde(default)]
    pub defaults: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ThemeTokens {
    #[serde(default)]
    pub color: ThemeColorTokens,
    #[serde(default)]
    pub typography: ThemeTypographyTokens,
    #[serde(default)]
    pub page: ThemePageTokens,
}

impl ThemeTokens {
    pub(super) fn is_empty(&self) -> bool {
        self.color.is_empty() && self.typography.is_empty() && self.page.is_empty()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ThemeColorTokens {
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub background: Option<String>,
    #[serde(default)]
    pub accent: Option<String>,
    #[serde(default)]
    pub muted: Option<String>,
    #[serde(default)]
    pub link: Option<String>,
    #[serde(default)]
    pub border: Option<String>,
    #[serde(default)]
    pub code_background: Option<String>,
}

impl ThemeColorTokens {
    fn is_empty(&self) -> bool {
        self.text.is_none()
            && self.background.is_none()
            && self.accent.is_none()
            && self.muted.is_none()
            && self.link.is_none()
            && self.border.is_none()
            && self.code_background.is_none()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ThemeTypographyTokens {
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub heading: Option<String>,
    #[serde(default)]
    pub mono: Option<String>,
    #[serde(default)]
    pub base_size_pt: Option<f64>,
    #[serde(default)]
    pub line_height: Option<f64>,
}

impl ThemeTypographyTokens {
    fn is_empty(&self) -> bool {
        self.body.is_none()
            && self.heading.is_none()
            && self.mono.is_none()
            && self.base_size_pt.is_none()
            && self.line_height.is_none()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ThemePageTokens {
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub margin_top_mm: Option<f64>,
    #[serde(default)]
    pub margin_right_mm: Option<f64>,
    #[serde(default)]
    pub margin_bottom_mm: Option<f64>,
    #[serde(default)]
    pub margin_left_mm: Option<f64>,
}

impl ThemePageTokens {
    fn is_empty(&self) -> bool {
        self.size.is_none()
            && self.margin_top_mm.is_none()
            && self.margin_right_mm.is_none()
            && self.margin_bottom_mm.is_none()
            && self.margin_left_mm.is_none()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginPackage {
    #[serde(default = "default_plugin_api_version")]
    pub api_version: u32,
    #[serde(default)]
    pub filters: Vec<PluginFilter>,
    #[serde(default)]
    pub commands: Vec<PluginCommand>,
}

fn default_plugin_api_version() -> u32 {
    PLUGIN_API_VERSION
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginFilter {
    pub script: String,
    #[serde(default)]
    pub formats: Vec<String>,
    #[serde(default = "default_filter_order")]
    pub order: i32,
    /// Optional globally unique key used by the filter to publish a depfile.
    /// OmniDoc exposes it as `omnidoc-plugin-depfile-<key>` and consumes
    /// `.omnidoc-cache/plugin-<key>.d` only while this filter is active.
    #[serde(default)]
    pub dependency_key: Option<String>,
}

fn default_filter_order() -> i32 {
    500
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginCommand {
    pub name: String,
    pub script: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PackageInspection {
    pub manifest_path: String,
    pub root: String,
    pub scope: PackageScope,
    pub source: String,
    pub digest: Option<String>,
    pub valid: bool,
    pub errors: Vec<String>,
    pub manifest: Option<PackageManifest>,
}

#[derive(Debug, Clone)]
pub(super) struct PackageRecord {
    pub root: PathBuf,
    pub scope: PackageScope,
    pub source: String,
    pub digest: String,
    pub manifest: PackageManifest,
}

impl PackageInspection {
    pub(super) fn into_record(self) -> Option<PackageRecord> {
        if !self.valid {
            return None;
        }
        Some(PackageRecord {
            root: PathBuf::from(self.root),
            scope: self.scope,
            source: self.source,
            digest: self.digest?,
            manifest: self.manifest?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct PackageSpec {
    pub id: String,
    pub requirement: VersionReq,
    pub raw_requirement: Option<String>,
}

impl PackageSpec {
    pub fn exact(id: &str, version: &str) -> String {
        format!("{id}@={version}")
    }

    pub(crate) fn exact_version(&self) -> Option<Version> {
        self.raw_requirement
            .as_deref()
            .and_then(|requirement| requirement.strip_prefix('='))
            .and_then(|version| Version::parse(version).ok())
    }

    pub(crate) fn matches_version(&self, version: &str) -> bool {
        let Ok(version) = Version::parse(version) else {
            return false;
        };
        self.exact_version()
            .map(|exact| version == exact)
            .unwrap_or_else(|| self.requirement.matches(&version))
    }
}

#[derive(Debug)]
pub struct InstallPackageRequest<'a> {
    pub expected_kind: PackageKind,
    pub source: &'a str,
    pub expected_sha256: Option<&'a str>,
    pub project_root: Option<&'a Path>,
    pub config: &'a MergedConfig,
    pub replace: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstallPackageReport {
    pub schema_version: u32,
    pub kind: PackageKind,
    pub id: String,
    pub version: String,
    pub source: String,
    pub destination: String,
    pub digest: String,
    pub archive_sha256: Option<String>,
    pub scope: PackageScope,
    pub installed: bool,
    pub replaced: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct UninstallPackageReport {
    pub schema_version: u32,
    pub kind: PackageKind,
    pub id: String,
    pub version: String,
    pub path: String,
    pub scope: PackageScope,
    pub removed: bool,
}

#[must_use]
pub struct ExtensionStoreReadLocks {
    _locks: Vec<ExtensionStoreLock>,
}

struct ExtensionStoreLock {
    file: fs::File,
}

impl Drop for ExtensionStoreLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InstallReceipt {
    receipt_version: u32,
    source: String,
    archive_sha256: Option<String>,
    payload_digest: String,
    installed_at_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InstallTransactionRecord {
    transaction_version: u32,
    destination: String,
    payload_digest: String,
}

#[derive(Debug, Clone)]
pub struct ExtensionResource {
    pub logical_name: String,
    pub resolved_from: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ResolvedPackageIdentity {
    pub kind: PackageKind,
    pub scope: PackageScope,
    pub id: String,
    pub version: String,
    pub source: String,
    pub digest: String,
    pub root: PathBuf,
    pub tracked_files: Vec<PathBuf>,
    pub compatible_pandoc: Option<String>,
}

pub fn package_spec(input: &str) -> Result<PackageSpec> {
    let input = input.trim();
    if input.is_empty() {
        return Err(OmniDocError::Config(
            "extension package specification must not be empty".to_string(),
        ));
    }
    let (id, raw_requirement) = match input.rsplit_once('@') {
        Some((id, requirement)) if !id.is_empty() && !requirement.is_empty() => {
            (id, Some(requirement))
        }
        _ => (input, None),
    };
    if !valid_package_id(id) {
        return Err(OmniDocError::Config(format!(
            "invalid extension package id '{id}'; use lower-case letters, numbers, '.', '_', '-', and optional '/' namespace separators, but do not use a semantic version as an ID segment"
        )));
    }
    let requirement = raw_requirement
        .map(VersionReq::parse)
        .transpose()
        .map_err(|error| {
            OmniDocError::Config(format!(
                "invalid version requirement in package specification '{input}': {error}"
            ))
        })?
        .unwrap_or(VersionReq::STAR);
    Ok(PackageSpec {
        id: id.to_string(),
        requirement,
        raw_requirement: raw_requirement.map(str::to_string),
    })
}

pub(super) fn valid_package_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 200
        && id.split('/').count() <= 16
        && id.split('/').all(|segment| {
            !segment.is_empty()
                && segment.len() <= 64
                && segment.chars().all(|character| {
                    character.is_ascii_lowercase()
                        || character.is_ascii_digit()
                        || matches!(character, '-' | '_' | '.')
                })
                && segment.chars().next().is_some_and(|character| {
                    character.is_ascii_lowercase() || character.is_ascii_digit()
                })
                && segment.chars().last().is_some_and(|character| {
                    character.is_ascii_lowercase() || character.is_ascii_digit()
                })
                // The on-disk layout is <kind>/<id segments>/<version>. A
                // semver-shaped ID segment would make one package's version
                // directory a valid namespace root for another package and
                // allow parent uninstall/replacement to remove the child.
                && Version::parse(segment).is_err()
                && portable_component(segment)
        })
}

fn valid_package_version(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value == value.to_ascii_lowercase()
        && portable_component(value)
        && Version::parse(value).is_ok()
}

pub(super) fn safe_relative_path(value: &str) -> Option<PathBuf> {
    if value.is_empty() || value.starts_with('/') || value.contains('\\') {
        return None;
    }
    let components = value.split('/').collect::<Vec<_>>();
    if components
        .iter()
        .any(|component| !portable_component(component))
    {
        return None;
    }
    Some(
        components
            .into_iter()
            .fold(PathBuf::new(), |path, component| path.join(component)),
    )
}

fn portable_component(component: &str) -> bool {
    !component.is_empty()
        && !matches!(component, "." | "..")
        && !component.ends_with([' ', '.'])
        && !component.chars().any(|character| {
            character.is_control()
                || matches!(
                    character,
                    '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
                )
        })
        && !windows_reserved_component(component)
}

fn windows_reserved_component(component: &str) -> bool {
    let basename = component
        .trim_end_matches([' ', '.'])
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    matches!(basename.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$")
        || basename
            .strip_prefix("COM")
            .or_else(|| basename.strip_prefix("LPT"))
            .is_some_and(|suffix| suffix.len() == 1 && matches!(suffix.as_bytes()[0], b'1'..=b'9'))
}

fn portable_path_text(path: &Path) -> Result<String> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(OmniDocError::Other(format!(
            "unsafe or non-portable extension package path: {}",
            path.display()
        )));
    }
    let mut components = Vec::new();
    for component in path.components() {
        let Component::Normal(component) = component else {
            return Err(OmniDocError::Other(format!(
                "unsafe or non-portable extension package path: {}",
                path.display()
            )));
        };
        let component = component.to_str().ok_or_else(|| {
            OmniDocError::Other(format!(
                "extension package paths must use UTF-8: {}",
                path.display()
            ))
        })?;
        if !portable_component(component) {
            return Err(OmniDocError::Other(format!(
                "unsafe or non-portable extension package path: {}",
                path.display()
            )));
        }
        components.push(component);
    }
    Ok(components.join("/"))
}

#[derive(Default)]
struct PortablePathSet {
    exact_paths: BTreeSet<String>,
    folded_prefixes: BTreeMap<String, String>,
}

impl PortablePathSet {
    fn insert(&mut self, path: &str) -> Result<()> {
        if !self.exact_paths.insert(path.to_string()) {
            return Err(OmniDocError::Other(format!(
                "duplicate extension package path: {path}"
            )));
        }
        let mut exact_prefix = String::new();
        let mut folded_prefix = String::new();
        for component in path.split('/') {
            if !exact_prefix.is_empty() {
                exact_prefix.push('/');
                folded_prefix.push('/');
            }
            exact_prefix.push_str(component);
            folded_prefix.push_str(&component.to_lowercase());
            if let Some(existing) = self.folded_prefixes.get(&folded_prefix) {
                if existing != &exact_prefix {
                    return Err(OmniDocError::Other(format!(
                        "extension package paths collide on case-insensitive filesystems: '{}' and '{}'",
                        existing, exact_prefix
                    )));
                }
            } else {
                self.folded_prefixes
                    .insert(folded_prefix.clone(), exact_prefix.clone());
            }
        }
        Ok(())
    }
}

pub(super) fn user_store_root(config: &MergedConfig) -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("OMNIDOC_EXTENSIONS_DIR").filter(|path| !path.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    if let Some(path) = config
        .extension_path
        .as_deref()
        .filter(|path| !path.trim().is_empty())
    {
        return Ok(PathBuf::from(path));
    }
    data_local_dir()
        .map(|path| path.join("omnidoc-extensions"))
        .ok_or_else(|| OmniDocError::Config("Local data directory not found".to_string()))
}

pub(crate) fn extension_store_roots(
    project_root: Option<&Path>,
    config: &MergedConfig,
) -> Result<Vec<PathBuf>> {
    let mut stores = vec![user_store_root(config)?];
    if let Some(project_root) = project_root {
        let store = project_store_root(project_root);
        validate_project_store_path(project_root, &store)?;
        stores.push(store);
    }
    stores.sort();
    stores.dedup();
    Ok(stores)
}

pub fn acquire_extension_store_read_locks(
    project_root: Option<&Path>,
    config: &MergedConfig,
    operation: &str,
) -> Result<ExtensionStoreReadLocks> {
    let project_store = project_root.map(project_store_root);
    let stores = extension_store_roots(project_root, config)?;
    let mut locks = Vec::with_capacity(stores.len());
    for store in stores {
        fs::create_dir_all(&store)?;
        if project_store.as_ref() == Some(&store) {
            let project_root = project_root.ok_or_else(|| {
                OmniDocError::Other("project-local extension store has no project root".to_string())
            })?;
            validate_project_store_path(project_root, &store)?;
        }
        let lock = acquire_extension_store_lock(&store, false, operation)?;
        ensure_no_interrupted_extension_transaction(&store, operation)?;
        locks.push(lock);
    }
    Ok(ExtensionStoreReadLocks { _locks: locks })
}

fn acquire_extension_store_lock(
    store: &Path,
    exclusive: bool,
    operation: &str,
) -> Result<ExtensionStoreLock> {
    fs::create_dir_all(store)?;
    let path = store.join(STORE_LOCK_FILE);
    if path
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err(OmniDocError::Other(format!(
            "extension store lock must not be a symbolic link: {}",
            path.display()
        )));
    }
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&path)?;
    let lock_result = if exclusive {
        FileExt::try_lock_exclusive(&file)
    } else {
        FileExt::try_lock_shared(&file)
    };
    lock_result.map_err(|error| {
        OmniDocError::Other(format!(
            "cannot {operation}: another OmniDoc process is using extension store {} ({error})",
            store.display()
        ))
    })?;
    Ok(ExtensionStoreLock { file })
}

fn ensure_no_interrupted_extension_transaction(store: &Path, operation: &str) -> Result<()> {
    let transaction_root = store.join(".transactions");
    if transaction_root
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err(OmniDocError::Other(format!(
            "extension transaction store must not be a symbolic link: {}",
            transaction_root.display()
        )));
    }
    if !transaction_root.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(&transaction_root)? {
        let entry = entry?;
        if entry.path().join("backup").exists() {
            return Err(OmniDocError::Other(format!(
                "cannot {operation}: extension store {} contains an interrupted replacement; run a plugin/theme install or uninstall command to recover it",
                store.display()
            )));
        }
    }
    Ok(())
}

fn recover_extension_transactions(store: &Path, transaction_root: &Path) -> Result<()> {
    if transaction_root
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err(OmniDocError::Other(format!(
            "extension transaction store must not be a symbolic link: {}",
            transaction_root.display()
        )));
    }
    if !transaction_root.is_dir() {
        return Ok(());
    }

    let mut entries =
        fs::read_dir(transaction_root)?.collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        let staging = entry.path();
        let metadata = entry.file_type()?;
        if metadata.is_symlink() || !metadata.is_dir() {
            return Err(OmniDocError::Other(format!(
                "unexpected extension transaction entry: {}",
                staging.display()
            )));
        }
        let backup = staging.join("backup");
        if backup
            .symlink_metadata()
            .is_ok_and(|metadata| metadata.file_type().is_symlink())
        {
            return Err(OmniDocError::Other(format!(
                "extension transaction backup must not be a symbolic link: {}",
                backup.display()
            )));
        }
        if backup.exists() {
            if !backup.is_dir() {
                return Err(OmniDocError::Other(format!(
                    "extension transaction backup is not a directory: {}",
                    backup.display()
                )));
            }
            let record_path = staging.join(TRANSACTION_RECORD_FILE);
            let record: InstallTransactionRecord =
                serde_json::from_str(&fs::read_to_string(&record_path).map_err(|error| {
                    OmniDocError::Other(format!(
                        "cannot recover extension transaction {}: {error}; backup retained at {}",
                        staging.display(),
                        backup.display()
                    ))
                })?)
                .map_err(|error| {
                    OmniDocError::Other(format!(
                    "cannot parse extension transaction record {}: {error}; backup retained at {}",
                    record_path.display(),
                    backup.display()
                ))
                })?;
            if record.transaction_version != TRANSACTION_RECORD_VERSION {
                return Err(OmniDocError::Other(format!(
                    "unsupported extension transaction version {} in {}; backup retained at {}",
                    record.transaction_version,
                    record_path.display(),
                    backup.display()
                )));
            }
            let destination = validated_transaction_destination(store, &record.destination)?;
            if destination.exists() {
                let destination_digest = directory_digest(&destination).map_err(|error| {
                    OmniDocError::Other(format!(
                        "cannot verify interrupted extension replacement at {}: {error}; backup retained at {}",
                        destination.display(),
                        backup.display()
                    ))
                })?;
                if destination_digest != record.payload_digest {
                    return Err(OmniDocError::Other(format!(
                        "interrupted extension replacement destination {} has digest {}, expected {}; backup retained at {}",
                        destination.display(),
                        destination_digest,
                        record.payload_digest,
                        backup.display()
                    )));
                }
                fs::remove_dir_all(&backup)?;
            } else {
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::rename(&backup, &destination).map_err(|error| {
                    OmniDocError::Other(format!(
                        "failed to restore interrupted extension package to {}: {error}; backup retained at {}",
                        destination.display(),
                        backup.display()
                    ))
                })?;
            }
        }
        fs::remove_dir_all(&staging)?;
    }
    Ok(())
}

fn validated_transaction_destination(store: &Path, value: &str) -> Result<PathBuf> {
    let relative = safe_relative_path(value).ok_or_else(|| {
        OmniDocError::Other(format!("unsafe extension transaction destination: {value}"))
    })?;
    let components = value.split('/').collect::<Vec<_>>();
    if components.len() < 3 {
        return Err(OmniDocError::Other(format!(
            "invalid extension transaction destination: {value}"
        )));
    }
    let directory = components[0];
    if !matches!(directory, "plugins" | "themes") {
        return Err(OmniDocError::Other(format!(
            "invalid extension transaction package kind: {value}"
        )));
    }
    let version = components.last().copied().unwrap_or_default();
    let id = components[1..components.len() - 1].join("/");
    if !valid_package_id(&id) || !valid_package_version(version) {
        return Err(OmniDocError::Other(format!(
            "invalid extension transaction package identity: {value}"
        )));
    }
    let kind_root = store.join(directory);
    fs::create_dir_all(&kind_root)?;
    reject_symlink(&kind_root, "extension package store")?;
    let destination = package_destination(&kind_root, &id, version)?;
    if destination != store.join(relative) {
        return Err(OmniDocError::Other(format!(
            "extension transaction destination is not canonical: {value}"
        )));
    }
    validate_store_destination(&kind_root, &destination)?;
    Ok(destination)
}

pub fn ensure_pandoc_compatible(
    packages: &[ResolvedPackageIdentity],
    config: &MergedConfig,
) -> Result<()> {
    let requirements = packages
        .iter()
        .filter_map(|package| {
            package.compatible_pandoc.as_deref().map(|requirement| {
                VersionReq::parse(requirement)
                    .map(|requirement| (package, requirement))
                    .map_err(|error| {
                        OmniDocError::Config(format!(
                            "invalid Pandoc compatibility range for {} '{}@{}': {error}",
                            package.kind.label(),
                            package.id,
                            package.version
                        ))
                    })
            })
        })
        .collect::<Result<Vec<_>>>()?;
    if requirements.is_empty()
        || requirements
            .iter()
            .all(|(_, requirement)| requirement == &VersionReq::STAR)
    {
        return Ok(());
    }
    let pandoc = config
        .tool_paths
        .get("pandoc")
        .and_then(|value| value.clone())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "pandoc".to_string());
    let output = Command::new(&pandoc)
        .arg("--version")
        .output()
        .map_err(|error| {
            OmniDocError::Config(format!(
                "cannot check extension Pandoc compatibility with '{}': {error}",
                pandoc
            ))
        })?;
    if !output.status.success() {
        return Err(OmniDocError::Config(format!(
            "cannot check extension Pandoc compatibility because '{}' --version exited with {}",
            pandoc, output.status
        )));
    }
    let version_text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let version = parse_pandoc_version(&version_text).ok_or_else(|| {
        OmniDocError::Config(format!(
            "cannot parse Pandoc version reported by '{}': {}",
            pandoc,
            version_text.lines().next().unwrap_or("empty output").trim()
        ))
    })?;
    let mut incompatible = Vec::new();
    for (package, requirement) in requirements {
        if !requirement.matches(&version) {
            incompatible.push(format!(
                "{} '{}@{}' requires Pandoc {}, installed {}",
                package.kind.label(),
                package.id,
                package.version,
                requirement,
                version
            ));
        }
    }
    if incompatible.is_empty() {
        Ok(())
    } else {
        Err(OmniDocError::Config(incompatible.join("; ")))
    }
}

fn parse_pandoc_version(output: &str) -> Option<Version> {
    for line in output.lines() {
        let tokens = line.split_whitespace().collect::<Vec<_>>();
        if let Some(index) = tokens.iter().position(|token| {
            let executable = token
                .trim_matches(|character: char| !character.is_ascii_alphanumeric())
                .to_ascii_lowercase();
            matches!(executable.as_str(), "pandoc" | "pandoc.exe")
        }) {
            if let Some(version) = tokens[index + 1..]
                .iter()
                .find_map(|token| parse_loose_version_token(token))
            {
                return Some(version);
            }
        }
    }
    None
}

fn parse_loose_version_token(token: &str) -> Option<Version> {
    let token = token.trim_start_matches(['v', 'V']);
    let numeric = token
        .chars()
        .take_while(|character| character.is_ascii_digit() || *character == '.')
        .collect::<String>();
    if numeric.is_empty() {
        return None;
    }
    let components = numeric
        .trim_end_matches('.')
        .split('.')
        .map(str::parse::<u64>)
        .collect::<std::result::Result<Vec<_>, _>>()
        .ok()?;
    let major = *components.first()?;
    let minor = components.get(1).copied().unwrap_or(0);
    let patch = components.get(2).copied().unwrap_or(0);
    Some(Version::new(major, minor, patch))
}

pub(super) fn project_store_root(project_root: &Path) -> PathBuf {
    project_root.join(".omnidoc").join("extensions")
}

pub fn discover_packages(
    kind: PackageKind,
    project_root: Option<&Path>,
    config: &MergedConfig,
) -> Result<Vec<PackageInspection>> {
    let mut roots = Vec::new();
    if let Some(project_root) = project_root {
        roots.push((
            PackageScope::Project,
            project_store_root(project_root).join(kind.directory()),
        ));
    }
    let user_root = user_store_root(config)?.join(kind.directory());
    if !roots
        .iter()
        .any(|(_, root)| paths_refer_to_same_location(root, &user_root))
    {
        roots.push((PackageScope::User, user_root));
    }

    let mut inspections = Vec::new();
    for (scope, root) in roots {
        if root
            .symlink_metadata()
            .is_ok_and(|metadata| metadata.file_type().is_symlink())
        {
            return Err(OmniDocError::Other(format!(
                "extension {} store must not be a symbolic link: {}",
                kind.label(),
                root.display()
            )));
        }
        if !root.is_dir() {
            continue;
        }
        for entry in WalkDir::new(&root)
            .follow_links(false)
            .max_depth(20)
            .into_iter()
            .filter_map(std::result::Result::ok)
            .filter(|entry| entry.file_type().is_file())
            .filter(|entry| entry.file_name() == PACKAGE_MANIFEST_FILE)
        {
            let mut inspection = inspect_package(entry.path(), scope, Some(kind));
            if let Some(manifest) = inspection.manifest.as_ref() {
                if let Ok(expected) = package_destination(&root, &manifest.id, &manifest.version) {
                    let actual = entry.path().parent().unwrap_or_else(|| Path::new("."));
                    if actual != expected {
                        inspection.valid = false;
                        inspection.errors.push(format!(
                            "package is not installed at its canonical {}/ID/VERSION path; expected {}",
                            kind.directory(),
                            expected.display()
                        ));
                    }
                }
            }
            inspections.push(inspection);
        }
    }
    inspections.sort_by(|left, right| {
        let left_manifest = left.manifest.as_ref();
        let right_manifest = right.manifest.as_ref();
        left_manifest
            .map(|manifest| manifest.id.as_str())
            .cmp(&right_manifest.map(|manifest| manifest.id.as_str()))
            .then_with(|| {
                left_manifest
                    .map(|manifest| manifest.version.as_str())
                    .cmp(&right_manifest.map(|manifest| manifest.version.as_str()))
            })
            .then_with(|| left.scope.cmp(&right.scope))
            .then_with(|| left.manifest_path.cmp(&right.manifest_path))
    });
    Ok(inspections)
}

fn paths_refer_to_same_location(left: &Path, right: &Path) -> bool {
    left == right
        || left
            .canonicalize()
            .ok()
            .zip(right.canonicalize().ok())
            .is_some_and(|(left, right)| left == right)
}

pub(super) fn package_records(
    kind: PackageKind,
    project_root: Option<&Path>,
    config: &MergedConfig,
) -> Result<Vec<PackageRecord>> {
    Ok(discover_packages(kind, project_root, config)?
        .into_iter()
        .filter_map(PackageInspection::into_record)
        .collect())
}

fn inspect_package(
    manifest_path: &Path,
    scope: PackageScope,
    expected_kind: Option<PackageKind>,
) -> PackageInspection {
    let root = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let mut inspection = PackageInspection {
        manifest_path: manifest_path.to_string_lossy().to_string(),
        root: root.to_string_lossy().to_string(),
        scope,
        source: scope_source(scope, root),
        digest: None,
        valid: false,
        errors: Vec::new(),
        manifest: None,
    };
    if manifest_path
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        inspection
            .errors
            .push("package manifest must not be a symbolic link".to_string());
        return inspection;
    }
    let content = match fs::read_to_string(manifest_path) {
        Ok(content) => content,
        Err(error) => {
            inspection
                .errors
                .push(format!("cannot read package manifest: {error}"));
            return inspection;
        }
    };
    let manifest = match toml::from_str::<PackageManifest>(&content) {
        Ok(manifest) => manifest,
        Err(error) => {
            inspection
                .errors
                .push(format!("invalid package manifest: {error}"));
            return inspection;
        }
    };
    inspection.source = install_source(root).unwrap_or_else(|| scope_source(scope, root));
    inspection.errors = validate_package(root, &manifest, expected_kind);
    if inspection.errors.is_empty() {
        match directory_digest(root) {
            Ok(digest) => inspection.digest = Some(digest),
            Err(error) => inspection.errors.push(error.to_string()),
        }
    }
    inspection.valid = inspection.errors.is_empty();
    inspection.manifest = Some(manifest);
    inspection
}

fn scope_source(scope: PackageScope, root: &Path) -> String {
    match scope {
        PackageScope::Builtin => "builtin".to_string(),
        PackageScope::User => "user-store".to_string(),
        PackageScope::Project => format!(
            "project:{}",
            root.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("package")
        ),
    }
}

fn install_source(root: &Path) -> Option<String> {
    fs::read_to_string(root.join(INSTALL_RECEIPT_FILE))
        .ok()
        .and_then(|content| serde_json::from_str::<InstallReceipt>(&content).ok())
        .map(|receipt| receipt.source)
}

fn validate_package(
    root: &Path,
    manifest: &PackageManifest,
    expected_kind: Option<PackageKind>,
) -> Vec<String> {
    let mut errors = Vec::new();
    validate_package_layout(root, &mut errors);
    if manifest.manifest_version != PACKAGE_MANIFEST_VERSION {
        errors.push(format!(
            "unsupported package manifest_version {}; expected {}",
            manifest.manifest_version, PACKAGE_MANIFEST_VERSION
        ));
    }
    if expected_kind.is_some_and(|kind| kind != manifest.kind) {
        errors.push(format!(
            "expected a {} package, found {}",
            expected_kind.unwrap().label(),
            manifest.kind.label()
        ));
    }
    if !valid_package_id(&manifest.id) {
        errors.push(format!("invalid package id: {}", manifest.id));
    }
    if !valid_package_version(&manifest.version) {
        errors.push(format!(
            "invalid package version '{}'; use a lower-case portable semantic version of at most 128 characters",
            manifest.version
        ));
    }
    match VersionReq::parse(&manifest.compatible_omnidoc) {
        Ok(requirement) => match Version::parse(env!("CARGO_PKG_VERSION")) {
            Ok(version) if !requirement.matches(&version) => errors.push(format!(
                "package requires OmniDoc {}, installed {}",
                manifest.compatible_omnidoc, version
            )),
            Ok(_) => {}
            Err(error) => errors.push(format!("invalid installed OmniDoc version: {error}")),
        },
        Err(error) => errors.push(format!("invalid OmniDoc compatibility range: {error}")),
    }
    if let Some(requirement) = manifest.compatible_pandoc.as_deref() {
        if let Err(error) = VersionReq::parse(requirement) {
            errors.push(format!("invalid Pandoc compatibility range: {error}"));
        }
    } else if manifest.kind == PackageKind::Plugin {
        errors.push("plugin package must declare compatible_pandoc".to_string());
    }
    if manifest
        .name
        .as_deref()
        .is_some_and(|name| name.trim().is_empty())
    {
        errors.push("package name must not be empty".to_string());
    }
    if manifest
        .description
        .as_deref()
        .is_some_and(|description| description.trim().is_empty())
    {
        errors.push("package description must not be empty".to_string());
    }

    match manifest.kind {
        PackageKind::Theme => {
            if manifest.plugin.is_some() {
                errors.push("theme package must not declare a [plugin] section".to_string());
            }
            match manifest.theme.as_ref() {
                Some(theme) => validate_theme(root, theme, &mut errors),
                None => errors.push("theme package must declare a [theme] section".to_string()),
            }
        }
        PackageKind::Plugin => {
            if manifest.theme.is_some() {
                errors.push("plugin package must not declare a [theme] section".to_string());
            }
            match manifest.plugin.as_ref() {
                Some(plugin) => validate_plugin(root, plugin, &mut errors),
                None => errors.push("plugin package must declare a [plugin] section".to_string()),
            }
        }
    }

    let checksum_file = manifest.checksum_file.as_deref().or_else(|| {
        root.join("checksums.sha256")
            .is_file()
            .then_some("checksums.sha256")
    });
    if let Some(checksum_file) = checksum_file {
        match safe_relative_path(checksum_file) {
            Some(relative) => {
                let path = root.join(relative);
                if !path.is_file() {
                    errors.push(format!("missing package checksum file: {checksum_file}"));
                } else if let Err(error) = verify_package_checksums(root, &path) {
                    errors.push(error.to_string());
                }
            }
            None => errors.push(format!("unsafe package checksum path: {checksum_file}")),
        }
    }
    errors
}

fn validate_package_layout(root: &Path, errors: &mut Vec<String>) {
    let expected_manifest = root.join(PACKAGE_MANIFEST_FILE);
    let mut unexpected_manifests = Vec::new();
    for (index, entry) in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .enumerate()
    {
        if index > MAX_PACKAGE_ENTRIES {
            errors.push(format!(
                "extension package contains more than {} entries",
                MAX_PACKAGE_ENTRIES
            ));
            break;
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                errors.push(format!("cannot inspect extension package layout: {error}"));
                break;
            }
        };
        if entry.file_type().is_file()
            && entry.file_name() == PACKAGE_MANIFEST_FILE
            && entry.path() != expected_manifest
        {
            unexpected_manifests.push(entry.path().display().to_string());
        }
    }
    if !unexpected_manifests.is_empty() {
        errors.push(format!(
            "extension package must contain exactly one root {PACKAGE_MANIFEST_FILE}; unexpected manifest(s): {}",
            unexpected_manifests.join(", ")
        ));
    }

    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.filter_map(std::result::Result::ok) {
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if !name.eq_ignore_ascii_case(INSTALL_RECEIPT_FILE) {
            continue;
        }
        if name != INSTALL_RECEIPT_FILE {
            errors.push(format!(
                "package root path '{name}' conflicts with reserved installation metadata '{INSTALL_RECEIPT_FILE}' on case-insensitive filesystems"
            ));
            continue;
        }
        match entry.file_type() {
            Ok(file_type) if file_type.is_file() => {}
            Ok(_) => errors.push(format!(
                "reserved installation metadata must be a regular file: {INSTALL_RECEIPT_FILE}"
            )),
            Err(error) => errors.push(format!(
                "cannot inspect reserved installation metadata {INSTALL_RECEIPT_FILE}: {error}"
            )),
        }
    }
}

fn validate_theme(root: &Path, theme: &ThemePackage, errors: &mut Vec<String>) {
    if theme.api_version != THEME_API_VERSION {
        errors.push(format!(
            "unsupported theme api_version {}; expected {}",
            theme.api_version, THEME_API_VERSION
        ));
    }
    if let Some(parent) = theme.extends.as_deref() {
        if let Err(error) = package_spec(parent) {
            errors.push(format!("invalid parent theme specification: {error}"));
        }
    }
    let mut outputs = BTreeSet::new();
    if let Some(declared_outputs) = &theme.outputs {
        for output in declared_outputs {
            let normalized = normalized_output(output);
            if !supported_output(&normalized) {
                errors.push(format!("unsupported theme output: {output}"));
            } else if !outputs.insert(normalized) {
                errors.push(format!("duplicate theme output: {output}"));
            }
        }
    }
    let resources = &theme.resources;
    for (kind, values) in [
        ("html_css", &resources.html_css),
        ("epub_css", &resources.epub_css),
        ("latex_packages", &resources.latex_packages),
        ("latex_headers", &resources.latex_headers),
    ] {
        validate_resource_list(root, kind, values, errors);
    }
    for (kind, value) in [
        ("html_template", resources.html_template.as_deref()),
        ("epub_template", resources.epub_template.as_deref()),
        ("latex_template", resources.latex_template.as_deref()),
        (
            "docx_reference_doc",
            resources.docx_reference_doc.as_deref(),
        ),
        (
            "pptx_reference_doc",
            resources.pptx_reference_doc.as_deref(),
        ),
    ] {
        if let Some(value) = value {
            validate_resource(root, kind, value, errors);
        }
    }
    if resources.html_css.is_empty()
        && resources.epub_css.is_empty()
        && resources.latex_packages.is_empty()
        && resources.latex_headers.is_empty()
        && resources.html_template.is_none()
        && resources.epub_template.is_none()
        && resources.latex_template.is_none()
        && resources.docx_reference_doc.is_none()
        && resources.pptx_reference_doc.is_none()
        && theme.tokens.is_empty()
        && theme.extends.is_none()
    {
        errors.push("theme must declare tokens, resources, or a parent theme".to_string());
    }
    if theme.outputs.is_some() {
        validate_theme_output_contract(theme, &outputs, errors);
    }
    validate_tokens(&theme.tokens, errors);
    validate_unique_strings("font requirement", &theme.requirements.fonts, errors);
    validate_unique_strings(
        "system LaTeX package requirement",
        &theme.requirements.system_latex_packages,
        errors,
    );
    for (key, value) in &theme.metadata.defaults {
        if !valid_metadata_key(key) {
            errors.push(format!("invalid theme metadata key: {key}"));
        }
        if value
            .chars()
            .any(|character| matches!(character, '\0' | '\n' | '\r'))
        {
            errors.push(format!(
                "theme metadata value for '{key}' must be a single-line scalar"
            ));
        }
    }
}

fn validate_theme_output_contract(
    theme: &ThemePackage,
    outputs: &BTreeSet<String>,
    errors: &mut Vec<String>,
) {
    let resources = &theme.resources;
    for (output, declared) in [
        (
            "html",
            !resources.html_css.is_empty() || resources.html_template.is_some(),
        ),
        (
            "epub",
            !resources.epub_css.is_empty() || resources.epub_template.is_some(),
        ),
        ("docx", resources.docx_reference_doc.is_some()),
        ("pptx", resources.pptx_reference_doc.is_some()),
    ] {
        if declared && !outputs.contains(output) {
            errors.push(format!(
                "theme declares {output} resources but omits '{output}' from theme.outputs"
            ));
        }
    }
    if (!resources.latex_packages.is_empty()
        || !resources.latex_headers.is_empty()
        || resources.latex_template.is_some())
        && !outputs.contains("pdf")
        && !outputs.contains("latex")
    {
        errors.push(
            "theme declares LaTeX resources but omits both 'pdf' and 'latex' from theme.outputs"
                .to_string(),
        );
    }
    if !theme.tokens.is_empty()
        && !["html", "epub", "pdf", "latex"]
            .iter()
            .any(|output| outputs.contains(*output))
    {
        errors.push(
            "theme declares semantic tokens but theme.outputs contains no token-capable output"
                .to_string(),
        );
    }
    if theme.extends.is_some() {
        return;
    }
    for output in outputs {
        let supported = match output.as_str() {
            "html" => {
                !theme.tokens.is_empty()
                    || !resources.html_css.is_empty()
                    || resources.html_template.is_some()
            }
            "epub" => {
                !theme.tokens.is_empty()
                    || !resources.epub_css.is_empty()
                    || resources.epub_template.is_some()
            }
            "pdf" | "latex" => {
                !theme.tokens.is_empty()
                    || !resources.latex_packages.is_empty()
                    || !resources.latex_headers.is_empty()
                    || resources.latex_template.is_some()
            }
            "docx" => resources.docx_reference_doc.is_some(),
            "pptx" => resources.pptx_reference_doc.is_some(),
            _ => false,
        };
        if !supported {
            errors.push(format!(
                "theme.outputs declares '{output}' but the theme has no resource for that output"
            ));
        }
    }
}

fn validate_plugin(root: &Path, plugin: &PluginPackage, errors: &mut Vec<String>) {
    if plugin.api_version != PLUGIN_API_VERSION {
        errors.push(format!(
            "unsupported plugin api_version {}; expected {}",
            plugin.api_version, PLUGIN_API_VERSION
        ));
    }
    if plugin.filters.is_empty() && plugin.commands.is_empty() {
        errors.push("plugin must declare at least one filter or command".to_string());
    }
    let mut filter_paths = BTreeSet::new();
    let mut dependency_keys = BTreeSet::new();
    for filter in &plugin.filters {
        validate_lua_script(root, "filter", &filter.script, errors);
        if !filter_paths.insert(filter.script.to_lowercase()) {
            errors.push(format!("duplicate plugin filter: {}", filter.script));
        }
        if let Some(key) = filter.dependency_key.as_deref() {
            if !valid_dependency_key(key) {
                errors.push(format!(
                    "invalid plugin filter dependency_key '{}'; use 1-96 lower-case letters, numbers, '-' or '_'",
                    key
                ));
            } else if !dependency_keys.insert(key.to_string()) {
                errors.push(format!("duplicate plugin filter dependency_key: {key}"));
            }
        }
        let mut formats = BTreeSet::new();
        for format in &filter.formats {
            let normalized = normalized_output(format);
            if !supported_output(&normalized) {
                errors.push(format!(
                    "unsupported format '{}' for filter {}",
                    format, filter.script
                ));
            } else if !formats.insert(normalized) {
                errors.push(format!(
                    "duplicate format '{}' for filter {}",
                    format, filter.script
                ));
            }
        }
    }
    let mut command_names = BTreeSet::new();
    for command in &plugin.commands {
        if !valid_command_name(&command.name) {
            errors.push(format!("invalid plugin command name: {}", command.name));
        } else if !command_names.insert(command.name.to_ascii_lowercase()) {
            errors.push(format!("duplicate plugin command: {}", command.name));
        }
        validate_lua_script(root, "command", &command.script, errors);
    }
}

fn validate_lua_script(root: &Path, kind: &str, value: &str, errors: &mut Vec<String>) {
    if !value.to_ascii_lowercase().ends_with(".lua") {
        errors.push(format!("plugin {kind} must use a .lua script: {value}"));
    }
    validate_resource(root, kind, value, errors);
}

fn validate_resource_list(root: &Path, kind: &str, values: &[String], errors: &mut Vec<String>) {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value.to_ascii_lowercase()) {
            errors.push(format!("duplicate {kind} resource: {value}"));
            continue;
        }
        validate_resource(root, kind, value, errors);
    }
}

fn validate_resource(root: &Path, kind: &str, value: &str, errors: &mut Vec<String>) {
    let Some(relative) = safe_relative_path(value) else {
        errors.push(format!("unsafe {kind} resource path: {value}"));
        return;
    };
    let path = root.join(relative);
    match path.symlink_metadata() {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            errors.push(format!(
                "{kind} resource must not be a symbolic link: {value}"
            ));
        }
        Ok(metadata) if metadata.is_file() => {
            if let (Ok(canonical_root), Ok(canonical_path)) =
                (root.canonicalize(), path.canonicalize())
            {
                if !canonical_path.starts_with(canonical_root) {
                    errors.push(format!("{kind} resource escapes the package: {value}"));
                }
            }
        }
        _ => errors.push(format!("missing {kind} resource: {value}")),
    }
}

fn validate_tokens(tokens: &ThemeTokens, errors: &mut Vec<String>) {
    for (name, value) in [
        ("text", tokens.color.text.as_deref()),
        ("background", tokens.color.background.as_deref()),
        ("accent", tokens.color.accent.as_deref()),
        ("muted", tokens.color.muted.as_deref()),
        ("link", tokens.color.link.as_deref()),
        ("border", tokens.color.border.as_deref()),
        ("code_background", tokens.color.code_background.as_deref()),
    ] {
        if let Some(value) = value {
            if normalized_hex_color(value).is_none() {
                errors.push(format!(
                    "theme color token '{name}' must be a #RGB or #RRGGBB value"
                ));
            }
        }
    }
    for (name, value) in [
        ("body", tokens.typography.body.as_deref()),
        ("heading", tokens.typography.heading.as_deref()),
        ("mono", tokens.typography.mono.as_deref()),
    ] {
        if let Some(value) = value {
            if value.trim().is_empty()
                || value.chars().any(|character| {
                    matches!(
                        character,
                        '\0' | '\n'
                            | '\r'
                            | '{'
                            | '}'
                            | '\\'
                            | '%'
                            | '#'
                            | '$'
                            | '&'
                            | '_'
                            | '^'
                            | '~'
                    )
                })
            {
                errors.push(format!("unsafe theme typography token '{name}'"));
            }
        }
    }
    if tokens
        .typography
        .base_size_pt
        .is_some_and(|value| !(6.0..=72.0).contains(&value))
    {
        errors.push("theme typography base_size_pt must be between 6 and 72".to_string());
    }
    if tokens
        .typography
        .line_height
        .is_some_and(|value| !(0.8..=3.0).contains(&value))
    {
        errors.push("theme typography line_height must be between 0.8 and 3.0".to_string());
    }
    if let Some(size) = tokens.page.size.as_deref() {
        if !matches!(
            size.trim().to_ascii_lowercase().as_str(),
            "a4" | "a5" | "letter"
        ) {
            errors.push("theme page size must be a4, a5, or letter".to_string());
        }
    }
    for (name, value) in [
        ("margin_top_mm", tokens.page.margin_top_mm),
        ("margin_right_mm", tokens.page.margin_right_mm),
        ("margin_bottom_mm", tokens.page.margin_bottom_mm),
        ("margin_left_mm", tokens.page.margin_left_mm),
    ] {
        if value.is_some_and(|value| !(0.0..=100.0).contains(&value)) {
            errors.push(format!("theme page {name} must be between 0 and 100"));
        }
    }
}

fn validate_unique_strings(kind: &str, values: &[String], errors: &mut Vec<String>) {
    let mut seen = BTreeSet::new();
    for value in values {
        let normalized = value.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            errors.push(format!("{kind} must not be empty"));
        } else if !seen.insert(normalized) {
            errors.push(format!("duplicate {kind}: {value}"));
        }
    }
}

pub(super) fn normalized_hex_color(value: &str) -> Option<String> {
    let value = value.trim().strip_prefix('#')?;
    if value.len() == 3 && value.chars().all(|character| character.is_ascii_hexdigit()) {
        let mut expanded = String::with_capacity(6);
        for character in value.chars() {
            expanded.push(character);
            expanded.push(character);
        }
        return Some(expanded.to_ascii_uppercase());
    }
    (value.len() == 6 && value.chars().all(|character| character.is_ascii_hexdigit()))
        .then(|| value.to_ascii_uppercase())
}

fn valid_metadata_key(key: &str) -> bool {
    let mut characters = key.chars();
    characters
        .next()
        .is_some_and(|character| character.is_ascii_alphanumeric() || character == '_')
        && characters.all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
}

fn valid_command_name(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
        && value
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_lowercase())
}

fn valid_dependency_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '-' | '_')
        })
        && value
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_lowercase() || character.is_ascii_digit())
        && value
            .chars()
            .last()
            .is_some_and(|character| character.is_ascii_lowercase() || character.is_ascii_digit())
}

pub(super) fn normalized_output(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "html4" | "html5" => "html".to_string(),
        "epub2" | "epub3" => "epub".to_string(),
        "tex" => "latex".to_string(),
        "powerpoint" => "pptx".to_string(),
        other => other.to_string(),
    }
}

fn supported_output(value: &str) -> bool {
    matches!(value, "pdf" | "html" | "epub" | "docx" | "pptx" | "latex")
}

fn verify_package_checksums(root: &Path, checksum_path: &Path) -> Result<()> {
    let content = fs::read_to_string(checksum_path)?;
    let mut seen = PortablePathSet::default();
    for (index, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let (expected, relative) = line.split_once("  ").ok_or_else(|| {
            OmniDocError::Other(format!(
                "invalid package checksum line {} in {}",
                index + 1,
                checksum_path.display()
            ))
        })?;
        if expected.len() != 64
            || !expected
                .chars()
                .all(|character| character.is_ascii_hexdigit())
        {
            return Err(OmniDocError::Other(format!(
                "invalid SHA-256 on package checksum line {}",
                index + 1
            )));
        }
        let safe = safe_relative_path(relative).ok_or_else(|| {
            OmniDocError::Other(format!("unsafe package checksum path: {relative}"))
        })?;
        seen.insert(&portable_path_text(&safe)?)?;
        let path = root.join(safe);
        if !path.is_file() {
            return Err(OmniDocError::Other(format!(
                "package checksum references missing file: {relative}"
            )));
        }
        let actual = file_sha256(&path)?;
        if !actual.eq_ignore_ascii_case(expected) {
            return Err(OmniDocError::Other(format!(
                "package checksum mismatch: {relative}"
            )));
        }
    }
    Ok(())
}

pub fn install_package(request: InstallPackageRequest<'_>) -> Result<InstallPackageReport> {
    let scope = if request.project_root.is_some() {
        PackageScope::Project
    } else {
        PackageScope::User
    };
    let store = match request.project_root {
        Some(project_root) => project_store_root(project_root),
        None => user_store_root(request.config)?,
    };
    if let Some(project_root) = request.project_root {
        validate_project_store_path(project_root, &store)?;
    }
    let operation = format!("install a {} package", request.expected_kind.label());
    let _store_lock = acquire_extension_store_lock(&store, true, &operation)?;
    if let Some(project_root) = request.project_root {
        validate_project_store_path(project_root, &store)?;
    }
    let kind_root = store.join(request.expected_kind.directory());
    fs::create_dir_all(&kind_root)?;
    reject_symlink(&kind_root, "extension package store")?;
    if let Some(project_root) = request.project_root {
        validate_project_store_path(project_root, &kind_root)?;
    }
    let transaction_root = store.join(".transactions");
    fs::create_dir_all(&transaction_root)?;
    reject_symlink(&transaction_root, "extension transaction store")?;
    if let Some(project_root) = request.project_root {
        validate_project_store_path(project_root, &transaction_root)?;
    }
    recover_extension_transactions(&store, &transaction_root)?;
    let staging = transaction_root.join(format!(
        ".omnidoc-install-{}-{}-{}",
        std::process::id(),
        current_timestamp_unix(),
        unique_nonce()
    ));
    if staging.exists() {
        return Err(OmniDocError::Other(format!(
            "extension installation staging path already exists: {}",
            staging.display()
        )));
    }
    fs::create_dir(&staging)?;

    let mut preserve_staging = false;
    let result = (|| {
        let payload = staging.join("payload");
        let prepared = staging.join("prepared");
        fs::create_dir(&prepared)?;
        let (source_label, archive_sha256) =
            prepare_source(request.source, request.expected_sha256, &prepared)?;
        let source_root = locate_package_root(&prepared)?;
        copy_package_tree(&source_root, &payload)?;
        strip_install_receipt(&payload)?;
        let inspection = inspect_package(
            &payload.join(PACKAGE_MANIFEST_FILE),
            scope,
            Some(request.expected_kind),
        );
        if !inspection.valid {
            return Err(OmniDocError::Other(format!(
                "extension package validation failed: {}",
                inspection.errors.join("; ")
            )));
        }
        let manifest = inspection
            .manifest
            .ok_or_else(|| OmniDocError::Other("validated package has no manifest".to_string()))?;
        let digest = inspection
            .digest
            .ok_or_else(|| OmniDocError::Other("validated package has no digest".to_string()))?;
        let destination = package_destination(&kind_root, &manifest.id, &manifest.version)?;
        validate_store_destination(&kind_root, &destination)?;
        let receipt = InstallReceipt {
            receipt_version: 1,
            source: source_label.clone(),
            archive_sha256: archive_sha256.clone(),
            payload_digest: digest.clone(),
            installed_at_unix: current_timestamp_unix(),
        };
        crate::utils::fs::atomic_write(
            payload.join(INSTALL_RECEIPT_FILE),
            serde_json::to_vec_pretty(&receipt)
                .map_err(|error| OmniDocError::Other(error.to_string()))?,
        )?;

        if destination.exists() {
            let existing = directory_digest(&destination)?;
            if existing == digest {
                return Ok(InstallPackageReport {
                    schema_version: 1,
                    kind: manifest.kind,
                    id: manifest.id,
                    version: manifest.version,
                    source: source_label,
                    destination: destination.to_string_lossy().to_string(),
                    digest,
                    archive_sha256,
                    scope,
                    installed: false,
                    replaced: false,
                });
            }
            if !request.replace {
                return Err(OmniDocError::Other(format!(
                    "package {} {} is already installed with a different digest; bump the package version or pass --replace explicitly",
                    manifest.id, manifest.version
                )));
            }
        }

        let transaction_destination = portable_path_text(
            destination
                .strip_prefix(&store)
                .map_err(|error| OmniDocError::Other(error.to_string()))?,
        )?;
        let transaction = InstallTransactionRecord {
            transaction_version: TRANSACTION_RECORD_VERSION,
            destination: transaction_destination,
            payload_digest: digest.clone(),
        };
        crate::utils::fs::atomic_write(
            staging.join(TRANSACTION_RECORD_FILE),
            serde_json::to_vec_pretty(&transaction)
                .map_err(|error| OmniDocError::Other(error.to_string()))?,
        )?;
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        let replaced = destination.exists();
        if replaced {
            let backup = staging.join("backup");
            fs::rename(&destination, &backup)?;
            if let Err(error) = fs::rename(&payload, &destination) {
                if let Err(restore_error) = fs::rename(&backup, &destination) {
                    preserve_staging = true;
                    return Err(OmniDocError::Other(format!(
                        "failed to activate replacement package ({error}) and failed to restore the previous package ({restore_error}); the previous payload is retained at {}",
                        backup.display()
                    )));
                }
                return Err(OmniDocError::Io(error));
            }
            let _ = fs::remove_dir_all(backup);
        } else {
            fs::rename(&payload, &destination)?;
        }

        Ok(InstallPackageReport {
            schema_version: 1,
            kind: manifest.kind,
            id: manifest.id,
            version: manifest.version,
            source: source_label,
            destination: destination.to_string_lossy().to_string(),
            digest,
            archive_sha256,
            scope,
            installed: true,
            replaced,
        })
    })();
    if staging.exists() && !preserve_staging {
        let _ = fs::remove_dir_all(&staging);
    }
    prune_empty_parents(Some(&transaction_root), &store);
    result
}

pub fn uninstall_package(
    kind: PackageKind,
    requested: &str,
    project_root: Option<&Path>,
    config: &MergedConfig,
) -> Result<UninstallPackageReport> {
    let spec = package_spec(requested)?;
    let scope = if project_root.is_some() {
        PackageScope::Project
    } else {
        PackageScope::User
    };
    let store = match project_root {
        Some(project_root) => project_store_root(project_root),
        None => user_store_root(config)?,
    };
    if let Some(project_root) = project_root {
        validate_project_store_path(project_root, &store)?;
    }
    if !store.is_dir() {
        return Err(OmniDocError::Other(format!(
            "no installed {} package satisfies '{}' in the {} store",
            kind.label(),
            requested,
            store.join(kind.directory()).display()
        )));
    }
    let operation = format!("uninstall a {} package", kind.label());
    let _store_lock = acquire_extension_store_lock(&store, true, &operation)?;
    if let Some(project_root) = project_root {
        validate_project_store_path(project_root, &store)?;
    }
    let transaction_root = store.join(".transactions");
    recover_extension_transactions(&store, &transaction_root)?;
    let kind_root = store.join(kind.directory());
    if let Some(project_root) = project_root {
        validate_project_store_path(project_root, &kind_root)?;
    }
    let mut candidates = discover_packages(kind, project_root, config)?
        .into_iter()
        .filter(|inspection| inspection.scope == scope)
        .filter_map(|inspection| {
            let manifest = inspection.manifest?;
            if manifest.kind != kind || manifest.id != spec.id {
                return None;
            }
            let version = Version::parse(&manifest.version).ok()?;
            spec.matches_version(&manifest.version)
                .then_some((version, PathBuf::from(inspection.root)))
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        candidates = layout_uninstall_candidates(&kind_root, &spec)?;
    }
    let (version, root) = candidates
        .into_iter()
        .max_by(|left, right| left.0.cmp(&right.0))
        .ok_or_else(|| {
            OmniDocError::Other(format!(
                "no installed {} package satisfies '{}' in the {} store",
                kind.label(),
                requested,
                kind_root.display()
            ))
        })?;
    validate_store_destination(&kind_root, &root)?;
    let canonical_kind_root = kind_root.canonicalize()?;
    let canonical_package = root.canonicalize()?;
    if !canonical_package.starts_with(&canonical_kind_root)
        || canonical_package == canonical_kind_root
    {
        return Err(OmniDocError::Other(format!(
            "refusing to remove package outside its store: {}",
            root.display()
        )));
    }
    fs::remove_dir_all(&root)?;
    prune_empty_parents(root.parent(), &kind_root);
    prune_empty_parents(Some(&transaction_root), &store);
    Ok(UninstallPackageReport {
        schema_version: 1,
        kind,
        id: spec.id,
        version: version.to_string(),
        path: root.to_string_lossy().to_string(),
        scope,
        removed: true,
    })
}

fn layout_uninstall_candidates(
    kind_root: &Path,
    spec: &PackageSpec,
) -> Result<Vec<(Version, PathBuf)>> {
    let mut id_root = kind_root.to_path_buf();
    for segment in spec.id.split('/') {
        id_root.push(segment);
    }
    if !id_root.is_dir() {
        return Ok(Vec::new());
    }
    validate_store_destination(kind_root, &id_root)?;

    let mut candidates = Vec::new();
    for entry in fs::read_dir(&id_root)? {
        let entry = entry?;
        let Some(version_text) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if !valid_package_version(&version_text) {
            continue;
        }
        let version = Version::parse(&version_text).expect("validated semantic version");
        if !spec.matches_version(&version_text) {
            continue;
        }
        let root = entry.path();
        if root.join(PACKAGE_MANIFEST_FILE).symlink_metadata().is_err()
            && root.join(INSTALL_RECEIPT_FILE).symlink_metadata().is_err()
        {
            continue;
        }
        candidates.push((version, root));
    }
    Ok(candidates)
}

fn prune_empty_parents(mut current: Option<&Path>, stop: &Path) {
    while let Some(path) = current {
        if path == stop || !path.starts_with(stop) {
            break;
        }
        if fs::read_dir(path)
            .ok()
            .is_some_and(|mut entries| entries.next().is_none())
        {
            if fs::remove_dir(path).is_err() {
                break;
            }
            current = path.parent();
        } else {
            break;
        }
    }
}

fn package_destination(kind_root: &Path, id: &str, version: &str) -> Result<PathBuf> {
    if !valid_package_id(id) || !valid_package_version(version) {
        return Err(OmniDocError::Other(format!(
            "unsafe package destination identity: {id}@{version}"
        )));
    }
    let mut destination = kind_root.to_path_buf();
    for segment in id.split('/') {
        destination.push(segment);
    }
    destination.push(version);
    Ok(destination)
}

fn validate_store_destination(kind_root: &Path, destination: &Path) -> Result<()> {
    let relative = destination.strip_prefix(kind_root).map_err(|_| {
        OmniDocError::Other(format!(
            "extension package destination is outside its store: {}",
            destination.display()
        ))
    })?;
    let canonical_root = kind_root.canonicalize()?;
    let mut current = kind_root.to_path_buf();
    for component in relative.components() {
        current.push(component.as_os_str());
        let Ok(metadata) = current.symlink_metadata() else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            return Err(OmniDocError::Other(format!(
                "extension package destination must not traverse a symbolic link: {}",
                current.display()
            )));
        }
    }
    if destination.exists() {
        let canonical_destination = destination.canonicalize()?;
        if canonical_destination == canonical_root
            || !canonical_destination.starts_with(&canonical_root)
        {
            return Err(OmniDocError::Other(format!(
                "extension package destination resolves outside its store: {}",
                destination.display()
            )));
        }
    }
    Ok(())
}

fn prepare_source(
    source: &str,
    expected_sha256: Option<&str>,
    destination: &Path,
) -> Result<(String, Option<String>)> {
    let lower_source = source.to_ascii_lowercase();
    if lower_source.starts_with("https://") {
        let source_label = sanitized_remote_source(source);
        let expected = expected_sha256.ok_or_else(|| {
            OmniDocError::Other(
                "HTTPS extension installation requires --sha256 to pin the downloaded archive"
                    .to_string(),
            )
        })?;
        let client = reqwest::blocking::Client::builder()
            .timeout(DOWNLOAD_TIMEOUT)
            .https_only(true)
            .build()
            .map_err(|error| {
                OmniDocError::Other(format!("failed to create extension HTTP client: {error}"))
            })?;
        let response = client.get(source).send().map_err(|error| {
            OmniDocError::Other(format!(
                "failed to download extension package from {}: {}",
                source_label,
                error.without_url()
            ))
        })?;
        if !response.status().is_success() {
            return Err(OmniDocError::HttpError {
                status: response.status().as_u16(),
                url: source_label,
            });
        }
        if response.url().scheme() != "https" {
            return Err(OmniDocError::Other(format!(
                "extension package download redirected to a non-HTTPS URL: {}",
                sanitized_remote_source(response.url().as_str())
            )));
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_ARCHIVE_BYTES)
        {
            return Err(OmniDocError::Other(format!(
                "extension archive exceeds {} bytes",
                MAX_ARCHIVE_BYTES
            )));
        }
        let mut bytes = Vec::new();
        response
            .take(MAX_ARCHIVE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| {
                OmniDocError::Other(format!(
                    "failed to read extension package response: {error}"
                ))
            })?;
        if bytes.len() as u64 > MAX_ARCHIVE_BYTES {
            return Err(OmniDocError::Other(format!(
                "extension archive exceeds {} bytes",
                MAX_ARCHIVE_BYTES
            )));
        }
        let digest = bytes_sha256(&bytes);
        verify_expected_sha256(expected, &digest)?;
        extract_archive(source, &bytes, destination)?;
        return Ok((source_label, Some(format!("sha256:{digest}"))));
    }
    if lower_source.starts_with("http://") {
        return Err(OmniDocError::Other(
            "extension packages may only be downloaded over HTTPS".to_string(),
        ));
    }

    let source_path = PathBuf::from(source).canonicalize().map_err(|error| {
        OmniDocError::Other(format!(
            "extension source '{}' is not accessible: {error}",
            source
        ))
    })?;
    if source_path.is_dir() {
        if expected_sha256.is_some() {
            return Err(OmniDocError::Other(
                "--sha256 is only valid for package archives".to_string(),
            ));
        }
        copy_package_tree(&source_path, destination)?;
        return Ok((format!("path:{}", source_path.display()), None));
    }
    if source_path.file_name().and_then(|name| name.to_str()) == Some(PACKAGE_MANIFEST_FILE) {
        if expected_sha256.is_some() {
            return Err(OmniDocError::Other(
                "--sha256 is only valid for package archives".to_string(),
            ));
        }
        let root = source_path.parent().unwrap_or_else(|| Path::new("."));
        copy_package_tree(root, destination)?;
        return Ok((format!("path:{}", root.display()), None));
    }
    let metadata = fs::metadata(&source_path)?;
    if metadata.len() > MAX_ARCHIVE_BYTES {
        return Err(OmniDocError::Other(format!(
            "extension archive exceeds {} bytes",
            MAX_ARCHIVE_BYTES
        )));
    }
    let bytes = fs::read(&source_path)?;
    let digest = bytes_sha256(&bytes);
    if let Some(expected) = expected_sha256 {
        verify_expected_sha256(expected, &digest)?;
    }
    extract_archive(&source_path.to_string_lossy(), &bytes, destination)?;
    Ok((
        format!("path:{}", source_path.display()),
        Some(format!("sha256:{digest}")),
    ))
}

fn extract_archive(name: &str, bytes: &[u8], destination: &Path) -> Result<()> {
    let archive_name = name.split(['?', '#']).next().unwrap_or(name);
    let lower = archive_name.to_ascii_lowercase();
    if lower.ends_with(".zip") || lower.ends_with(".odpkg") {
        extract_zip(bytes, destination)
    } else if lower.ends_with(".tar.gz") || lower.ends_with(".tgz") {
        extract_tar_gz(bytes, destination)
    } else {
        Err(OmniDocError::Other(format!(
            "unsupported extension archive '{}'; use .zip, .odpkg, .tar.gz, or .tgz",
            name
        )))
    }
}

fn extract_zip(bytes: &[u8], destination: &Path) -> Result<()> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| OmniDocError::Other(format!("invalid ZIP package: {error}")))?;
    if archive.len() > MAX_PACKAGE_ENTRIES {
        return Err(OmniDocError::Other(format!(
            "extension archive contains more than {} entries",
            MAX_PACKAGE_ENTRIES
        )));
    }
    let mut total = 0u64;
    let mut paths = PortablePathSet::default();
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| OmniDocError::Other(format!("invalid ZIP entry: {error}")))?;
        let raw_name = std::str::from_utf8(entry.name_raw())
            .map_err(|_| OmniDocError::Other("ZIP package paths must use UTF-8".to_string()))?;
        let raw_name = if entry.is_dir() {
            raw_name.trim_end_matches('/')
        } else {
            raw_name
        };
        if raw_name.is_empty() && entry.is_dir() {
            continue;
        }
        let relative = safe_relative_path(raw_name)
            .ok_or_else(|| OmniDocError::Other(format!("unsafe ZIP path: {raw_name}")))?;
        let portable = portable_path_text(&relative)?;
        paths.insert(&portable)?;
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err(OmniDocError::Other(format!(
                "symbolic links are not allowed in extension packages: {}",
                portable
            )));
        }
        total = total
            .checked_add(entry.size())
            .ok_or_else(|| OmniDocError::Other("extension archive size overflow".to_string()))?;
        if total > MAX_EXTRACTED_BYTES {
            return Err(OmniDocError::Other(format!(
                "extension archive expands beyond {} bytes",
                MAX_EXTRACTED_BYTES
            )));
        }
        let output = destination.join(&relative);
        if entry.is_dir() {
            fs::create_dir_all(output)?;
            continue;
        }
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = fs::File::create(output)?;
        std::io::copy(&mut entry, &mut file)?;
        file.flush()?;
    }
    Ok(())
}

fn extract_tar_gz(bytes: &[u8], destination: &Path) -> Result<()> {
    let decoder = GzDecoder::new(Cursor::new(bytes));
    let mut archive = Archive::new(decoder);
    let mut total = 0u64;
    let mut paths = PortablePathSet::default();
    for (index, entry) in archive.entries().map_err(OmniDocError::Io)?.enumerate() {
        if index >= MAX_PACKAGE_ENTRIES {
            return Err(OmniDocError::Other(format!(
                "extension archive contains more than {} entries",
                MAX_PACKAGE_ENTRIES
            )));
        }
        let mut entry = entry.map_err(OmniDocError::Io)?;
        let entry_type = entry.header().entry_type();
        if !entry_type.is_file() && !entry_type.is_dir() {
            let path = entry.path().map_err(OmniDocError::Io)?;
            return Err(OmniDocError::Other(format!(
                "unsupported tar package entry: {}",
                path.display()
            )));
        }
        let raw_path = entry.path_bytes();
        let raw_path = std::str::from_utf8(raw_path.as_ref())
            .map_err(|_| OmniDocError::Other("tar package paths must use UTF-8".to_string()))?;
        let raw_path = if entry_type.is_dir() {
            raw_path.trim_end_matches('/')
        } else {
            raw_path
        };
        if raw_path.is_empty() && entry_type.is_dir() {
            continue;
        }
        let relative = safe_relative_path(raw_path)
            .ok_or_else(|| OmniDocError::Other(format!("unsafe tar package path: {raw_path}")))?;
        let portable = portable_path_text(&relative)?;
        paths.insert(&portable)?;
        total = total
            .checked_add(entry.header().size().unwrap_or(0))
            .ok_or_else(|| OmniDocError::Other("extension archive size overflow".to_string()))?;
        if total > MAX_EXTRACTED_BYTES {
            return Err(OmniDocError::Other(format!(
                "extension archive expands beyond {} bytes",
                MAX_EXTRACTED_BYTES
            )));
        }
        let output = destination.join(&relative);
        if entry_type.is_dir() {
            fs::create_dir_all(output)?;
        } else {
            if let Some(parent) = output.parent() {
                fs::create_dir_all(parent)?;
            }
            entry.unpack(output).map_err(OmniDocError::Io)?;
        }
    }
    Ok(())
}

fn locate_package_root(root: &Path) -> Result<PathBuf> {
    let mut manifests = Vec::new();
    for entry in WalkDir::new(root).follow_links(false) {
        let entry = entry.map_err(OmniDocError::Walkdir)?;
        if entry.file_type().is_file() && entry.file_name() == PACKAGE_MANIFEST_FILE {
            manifests.push(entry.path().to_path_buf());
            if manifests.len() > 1 {
                break;
            }
        }
    }
    match manifests.as_slice() {
        [manifest] => Ok(manifest
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf()),
        [] => Err(OmniDocError::Other(format!(
            "extension source does not contain {PACKAGE_MANIFEST_FILE}"
        ))),
        _ => Err(OmniDocError::Other(format!(
            "extension source contains multiple {PACKAGE_MANIFEST_FILE} files"
        ))),
    }
}

fn copy_package_tree(source: &Path, destination: &Path) -> Result<()> {
    if source
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err(OmniDocError::Other(format!(
            "package source must not be a symbolic link: {}",
            source.display()
        )));
    }
    fs::create_dir_all(destination)?;
    let mut total = 0u64;
    let mut paths = PortablePathSet::default();
    for (index, entry) in WalkDir::new(source)
        .follow_links(false)
        .into_iter()
        .enumerate()
    {
        if index > MAX_PACKAGE_ENTRIES {
            return Err(OmniDocError::Other(format!(
                "extension package contains more than {} entries",
                MAX_PACKAGE_ENTRIES
            )));
        }
        let entry = entry.map_err(OmniDocError::Walkdir)?;
        if entry.depth() > 0
            && entry
                .path()
                .components()
                .any(|component| component.as_os_str() == ".git")
        {
            continue;
        }
        if entry.file_type().is_symlink() {
            return Err(OmniDocError::Other(format!(
                "symbolic links are not allowed in extension packages: {}",
                entry.path().display()
            )));
        }
        let relative = entry
            .path()
            .strip_prefix(source)
            .map_err(|error| OmniDocError::Other(error.to_string()))?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        let portable = portable_path_text(relative)?;
        paths.insert(&portable)?;
        let output = destination.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(output)?;
        } else if entry.file_type().is_file() {
            let size = entry.metadata().map_err(OmniDocError::Walkdir)?.len();
            total = total.checked_add(size).ok_or_else(|| {
                OmniDocError::Other("extension package size overflow".to_string())
            })?;
            if total > MAX_EXTRACTED_BYTES {
                return Err(OmniDocError::Other(format!(
                    "extension package exceeds {} bytes",
                    MAX_EXTRACTED_BYTES
                )));
            }
            if let Some(parent) = output.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(entry.path(), output)?;
        } else {
            return Err(OmniDocError::Other(format!(
                "unsupported extension package entry: {}",
                entry.path().display()
            )));
        }
    }
    Ok(())
}

fn strip_install_receipt(root: &Path) -> Result<()> {
    let path = root.join(INSTALL_RECEIPT_FILE);
    match path.symlink_metadata() {
        Ok(metadata) if metadata.is_file() => fs::remove_file(path).map_err(OmniDocError::Io),
        Ok(_) => Err(OmniDocError::Other(format!(
            "reserved installation metadata must be a regular file: {}",
            path.display()
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(OmniDocError::Io(error)),
    }
}

pub(super) fn directory_digest(root: &Path) -> Result<String> {
    let mut paths = PortablePathSet::default();
    let mut entries = Vec::new();
    for (index, entry) in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .enumerate()
    {
        if index > MAX_PACKAGE_ENTRIES {
            return Err(OmniDocError::Other(format!(
                "extension package contains more than {} entries",
                MAX_PACKAGE_ENTRIES
            )));
        }
        let entry = entry.map_err(OmniDocError::Walkdir)?;
        let relative = entry
            .path()
            .strip_prefix(root)
            .map_err(|error| OmniDocError::Other(error.to_string()))?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        let portable = portable_path_text(relative)?;
        paths.insert(&portable)?;
        if entry.file_type().is_symlink() {
            return Err(OmniDocError::Other(format!(
                "symbolic links are not allowed in extension packages: {}",
                entry.path().display()
            )));
        }
        if entry.file_type().is_dir() {
            entries.push((portable, false, entry.path().to_path_buf()));
        } else if entry.file_type().is_file() {
            if relative == Path::new(INSTALL_RECEIPT_FILE) {
                continue;
            }
            entries.push((portable, true, entry.path().to_path_buf()));
        } else {
            return Err(OmniDocError::Other(format!(
                "unsupported extension package entry: {}",
                entry.path().display()
            )));
        }
    }
    entries.sort_by(|left, right| left.0.cmp(&right.0));

    let mut hasher = Sha256::new();
    hasher.update(PACKAGE_DIGEST_DOMAIN);
    hasher.update((entries.len() as u64).to_le_bytes());
    for (relative, is_file, path) in entries {
        hasher.update((relative.len() as u64).to_le_bytes());
        hasher.update(relative.as_bytes());
        hasher.update([u8::from(is_file)]);
        if !is_file {
            continue;
        }
        let metadata = fs::metadata(&path)?;
        hasher.update(metadata.len().to_le_bytes());
        let mut file = fs::File::open(&path)?;
        let mut buffer = [0u8; 64 * 1024];
        let mut total = 0u64;
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            total = total
                .checked_add(read as u64)
                .ok_or_else(|| OmniDocError::Other("extension file size overflow".to_string()))?;
            hasher.update(&buffer[..read]);
        }
        if total != metadata.len() {
            return Err(OmniDocError::Other(format!(
                "extension package changed while computing its digest: {}",
                path.display()
            )));
        }
    }
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

pub(super) fn tracked_package_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut paths = PortablePathSet::default();
    for (index, entry) in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .enumerate()
    {
        if index > MAX_PACKAGE_ENTRIES {
            return Err(OmniDocError::Other(format!(
                "extension package contains more than {} entries",
                MAX_PACKAGE_ENTRIES
            )));
        }
        let entry = entry.map_err(OmniDocError::Walkdir)?;
        let relative = entry
            .path()
            .strip_prefix(root)
            .map_err(|error| OmniDocError::Other(error.to_string()))?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        let portable = portable_path_text(relative)?;
        paths.insert(&portable)?;
        if entry.file_type().is_symlink() {
            return Err(OmniDocError::Other(format!(
                "symbolic links are not allowed in extension packages: {}",
                entry.path().display()
            )));
        }
        if entry.file_type().is_dir() {
            continue;
        }
        if !entry.file_type().is_file() {
            return Err(OmniDocError::Other(format!(
                "unsupported extension package entry: {}",
                entry.path().display()
            )));
        }
        if relative == Path::new(INSTALL_RECEIPT_FILE) {
            continue;
        }
        files.push((portable, entry.path().to_path_buf()));
    }
    files.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(files.into_iter().map(|(_, path)| path).collect())
}

pub(super) fn digest_files(root: &Path, files: &[PathBuf]) -> Result<String> {
    let mut paths = PortablePathSet::default();
    let mut ordered = Vec::with_capacity(files.len());
    for path in files {
        let relative = path.strip_prefix(root).map_err(|_| {
            OmniDocError::Other(format!(
                "cannot digest extension file outside package root: {}",
                path.display()
            ))
        })?;
        let portable = portable_path_text(relative)?;
        paths.insert(&portable)?;
        ordered.push((portable, path));
    }
    ordered.sort_by(|left, right| left.0.cmp(&right.0));

    let mut hasher = Sha256::new();
    hasher.update(PACKAGE_DIGEST_DOMAIN);
    hasher.update((ordered.len() as u64).to_le_bytes());
    for (relative, path) in ordered {
        let metadata = fs::metadata(path)?;
        if !metadata.is_file() {
            return Err(OmniDocError::Other(format!(
                "cannot digest non-file extension entry: {}",
                path.display()
            )));
        }
        hasher.update((relative.len() as u64).to_le_bytes());
        hasher.update(relative.as_bytes());
        hasher.update(metadata.len().to_le_bytes());
        let mut file = fs::File::open(path)?;
        let mut buffer = [0u8; 64 * 1024];
        let mut total = 0u64;
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            total = total
                .checked_add(read as u64)
                .ok_or_else(|| OmniDocError::Other("extension file size overflow".to_string()))?;
            hasher.update(&buffer[..read]);
        }
        if total != metadata.len() {
            return Err(OmniDocError::Other(format!(
                "extension package changed while computing its digest: {}",
                path.display()
            )));
        }
    }
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

fn file_sha256(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn bytes_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn sanitized_remote_source(source: &str) -> String {
    let Ok(mut url) = reqwest::Url::parse(source) else {
        return "https://invalid-extension-source".to_string();
    };
    let _ = url.set_password(None);
    let _ = url.set_username("");
    url.set_query(None);
    url.set_fragment(None);
    url.to_string()
}

fn verify_expected_sha256(expected: &str, actual: &str) -> Result<()> {
    let expected = expected
        .trim()
        .strip_prefix("sha256:")
        .unwrap_or(expected.trim());
    if expected.len() != 64
        || !expected
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(OmniDocError::Other(
            "--sha256 must contain exactly 64 hexadecimal characters".to_string(),
        ));
    }
    if !expected.eq_ignore_ascii_case(actual) {
        return Err(OmniDocError::Other(format!(
            "extension archive SHA-256 mismatch: expected {expected}, got {actual}"
        )));
    }
    Ok(())
}

fn reject_symlink(path: &Path, label: &str) -> Result<()> {
    if path
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err(OmniDocError::Other(format!(
            "{label} must not be a symbolic link: {}",
            path.display()
        )));
    }
    Ok(())
}

fn validate_project_store_path(project_root: &Path, target: &Path) -> Result<()> {
    let canonical_project = project_root.canonicalize().map_err(|error| {
        OmniDocError::Project(format!(
            "cannot resolve project root {}: {error}",
            project_root.display()
        ))
    })?;
    let relative = target.strip_prefix(project_root).map_err(|_| {
        OmniDocError::Project(format!(
            "project extension store is outside the project: {}",
            target.display()
        ))
    })?;
    let mut current = project_root.to_path_buf();
    for component in relative.components() {
        current.push(component.as_os_str());
        let Ok(metadata) = current.symlink_metadata() else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            return Err(OmniDocError::Project(format!(
                "project extension store must not traverse a symbolic link: {}",
                current.display()
            )));
        }
    }
    if target.exists() {
        let canonical_target = target.canonicalize()?;
        if !canonical_target.starts_with(&canonical_project) {
            return Err(OmniDocError::Project(format!(
                "project extension store resolves outside the project: {}",
                target.display()
            )));
        }
    }
    Ok(())
}

fn current_timestamp_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn unique_nonce() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{
        digest_files, directory_digest, discover_packages, ensure_pandoc_compatible, extract_zip,
        inspect_package, install_package, normalized_hex_color, package_spec, parse_pandoc_version,
        safe_relative_path, sanitized_remote_source, uninstall_package, valid_package_id,
        valid_package_version, validate_tokens, InstallPackageRequest, PackageKind, PackageScope,
        ResolvedPackageIdentity, ThemeTokens,
    };
    use crate::config::MergedConfig;
    use std::fs;
    use std::io::{Cursor, Write};
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    fn write_plugin_package(root: &std::path::Path, script: &str) {
        fs::create_dir_all(root.join("filters")).expect("filter directory");
        fs::write(root.join("filters/main.lua"), script).expect("filter script");
        fs::write(
            root.join(super::PACKAGE_MANIFEST_FILE),
            r#"manifest_version = 2
kind = "plugin"
id = "acme/check"
name = "ACME Check"
version = "1.2.0"
compatible_omnidoc = ">=1.8,<2"
compatible_pandoc = ">=3,<4"

[plugin]
api_version = 1

[[plugin.filters]]
script = "filters/main.lua"
formats = ["html", "pdf"]
order = 500
"#,
        )
        .expect("package manifest");
    }

    #[test]
    fn parses_namespaced_package_specs() {
        let spec = package_spec("acme/quality@^1.2").expect("package spec");
        assert_eq!(spec.id, "acme/quality");
        assert!(spec.requirement.matches(&semver::Version::new(1, 4, 0)));
        assert!(!spec.requirement.matches(&semver::Version::new(2, 0, 0)));
    }

    #[test]
    fn exact_package_specs_include_semver_build_metadata() {
        let exact = package_spec("acme/quality@=1.2.0+linux").expect("exact package spec");
        assert!(exact.matches_version("1.2.0+linux"));
        assert!(!exact.matches_version("1.2.0+windows"));
        assert!(!exact.matches_version("1.2.0"));

        let range = package_spec("acme/quality@^1.2").expect("range package spec");
        assert!(range.matches_version("1.2.0+linux"));
        assert!(range.matches_version("1.2.0+windows"));
    }

    #[test]
    fn validates_package_ids_and_relative_paths() {
        assert!(valid_package_id("acme/corporate-theme"));
        assert!(valid_package_id("3d/corporate-theme"));
        assert!(!valid_package_id("Acme/theme"));
        assert!(!valid_package_id("acme//theme"));
        assert!(!valid_package_id("acme/con"));
        assert!(!valid_package_id("acme/1.2.3/theme"));
        assert!(!valid_package_id("acme/2.0.0-beta/theme"));
        assert!(valid_package_version("1.2.3-beta.1+linux"));
        assert!(!valid_package_version("1.2.3-BETA"));
        assert!(safe_relative_path("filters/main.lua").is_some());
        assert!(safe_relative_path("../main.lua").is_none());
        assert!(safe_relative_path(r"filters\main.lua").is_none());
        assert!(safe_relative_path("filters/main.lua.").is_none());
    }

    #[test]
    fn parses_real_world_pandoc_versions() {
        assert_eq!(
            parse_pandoc_version("pandoc 3.1.11.1\nFeatures: +server"),
            Some(semver::Version::new(3, 1, 11))
        );
        assert_eq!(
            parse_pandoc_version("pandoc 3.10"),
            Some(semver::Version::new(3, 10, 0))
        );
        assert_eq!(
            parse_pandoc_version("Copyright 2026\npandoc.exe 3.7.0.2"),
            Some(semver::Version::new(3, 7, 0))
        );
        assert_eq!(
            parse_pandoc_version("unrelated tool 3.7.0\nCopyright 2026"),
            None
        );
        assert_eq!(parse_pandoc_version("pandoc-crossref 0.3.18.0"), None);
        assert_eq!(parse_pandoc_version("pandoc-server 3.7.0"), None);
        assert_eq!(parse_pandoc_version("pandoc 3..7"), None);
    }

    #[test]
    fn plugin_manifest_requires_a_pandoc_compatibility_range() {
        let package = tempfile::tempdir().expect("package");
        write_plugin_package(package.path(), "return {}\n");
        let manifest_path = package.path().join(super::PACKAGE_MANIFEST_FILE);
        let manifest = fs::read_to_string(&manifest_path)
            .expect("plugin manifest")
            .replace("compatible_pandoc = \">=3,<4\"\n", "");
        fs::write(&manifest_path, manifest).expect("manifest without Pandoc range");

        let inspection = inspect_package(
            &manifest_path,
            PackageScope::User,
            Some(PackageKind::Plugin),
        );
        assert!(!inspection.valid);
        assert!(inspection
            .errors
            .iter()
            .any(|error| error.contains("must declare compatible_pandoc")));
    }

    #[test]
    fn normalizes_short_and_long_hex_colors() {
        assert_eq!(normalized_hex_color("#0af").as_deref(), Some("00AAFF"));
        assert_eq!(normalized_hex_color("#12abEF").as_deref(), Some("12ABEF"));
        assert!(normalized_hex_color("red").is_none());
    }

    #[test]
    fn typography_tokens_reject_tex_control_characters() {
        let mut valid = ThemeTokens::default();
        valid.typography.body = Some("Noto Serif CJK SC".to_string());
        let mut errors = Vec::new();
        validate_tokens(&valid, &mut errors);
        assert!(errors.is_empty());

        for value in ["Family_Name", "Family & Sons", "Family#1", "Family~Alt"] {
            let mut tokens = ThemeTokens::default();
            tokens.typography.body = Some(value.to_string());
            let mut errors = Vec::new();
            validate_tokens(&tokens, &mut errors);
            assert!(errors
                .iter()
                .any(|error| error.contains("unsafe theme typography token")));
        }
    }

    #[test]
    fn remote_source_labels_do_not_persist_credentials_or_query_tokens() {
        assert_eq!(
            sanitized_remote_source(
                "https://user:secret@example.com/releases/plugin.zip?token=signed#download"
            ),
            "https://example.com/releases/plugin.zip"
        );
    }

    #[test]
    fn installs_discovers_replaces_and_uninstalls_versioned_packages() {
        let workspace = tempfile::tempdir().expect("workspace");
        let source = workspace.path().join("source");
        let store = workspace.path().join("store");
        write_plugin_package(&source, "return {}\n");
        let config = MergedConfig {
            extension_path: Some(store.to_string_lossy().to_string()),
            ..Default::default()
        };
        let source_text = source.to_string_lossy().to_string();

        let first = install_package(InstallPackageRequest {
            expected_kind: PackageKind::Plugin,
            source: &source_text,
            expected_sha256: None,
            project_root: None,
            config: &config,
            replace: false,
        })
        .expect("first install");
        assert!(first.installed);
        assert!(!first.replaced);
        assert!(!store.join(".transactions").exists());

        let second = install_package(InstallPackageRequest {
            expected_kind: PackageKind::Plugin,
            source: &source_text,
            expected_sha256: None,
            project_root: None,
            config: &config,
            replace: false,
        })
        .expect("idempotent install");
        assert!(!second.installed);
        assert_eq!(second.digest, first.digest);

        fs::write(
            source.join("filters/main.lua"),
            "return { Pandoc = function(doc) return doc end }\n",
        )
        .expect("updated filter");
        let conflict = install_package(InstallPackageRequest {
            expected_kind: PackageKind::Plugin,
            source: &source_text,
            expected_sha256: None,
            project_root: None,
            config: &config,
            replace: false,
        })
        .expect_err("replacement must be explicit");
        assert!(conflict.to_string().contains("different digest"));

        let replaced = install_package(InstallPackageRequest {
            expected_kind: PackageKind::Plugin,
            source: &source_text,
            expected_sha256: None,
            project_root: None,
            config: &config,
            replace: true,
        })
        .expect("explicit replacement");
        assert!(replaced.installed);
        assert!(replaced.replaced);
        assert_ne!(replaced.digest, first.digest);

        let discovered =
            discover_packages(PackageKind::Plugin, None, &config).expect("package discovery");
        assert_eq!(discovered.len(), 1);
        assert!(discovered[0].valid);
        assert_eq!(
            discovered[0]
                .manifest
                .as_ref()
                .map(|manifest| manifest.id.as_str()),
            Some("acme/check")
        );

        let removed = uninstall_package(PackageKind::Plugin, "acme/check@=1.2.0", None, &config)
            .expect("uninstall package");
        assert!(removed.removed);
        assert!(discover_packages(PackageKind::Plugin, None, &config)
            .expect("empty discovery")
            .is_empty());
    }

    #[test]
    fn installation_rejects_nested_package_manifests_at_any_depth() {
        let workspace = tempfile::tempdir().expect("workspace");
        let source = workspace.path().join("source");
        let store = workspace.path().join("store");
        write_plugin_package(&source, "return {}\n");
        let nested = source.join("assets/examples/deeply/nested/package");
        fs::create_dir_all(&nested).expect("nested package directory");
        fs::write(
            nested.join(super::PACKAGE_MANIFEST_FILE),
            "manifest_version = 2\n",
        )
        .expect("nested manifest");
        let config = MergedConfig {
            extension_path: Some(store.to_string_lossy().to_string()),
            ..Default::default()
        };
        let source = source.to_string_lossy().to_string();

        let error = install_package(InstallPackageRequest {
            expected_kind: PackageKind::Plugin,
            source: &source,
            expected_sha256: None,
            project_root: None,
            config: &config,
            replace: false,
        })
        .expect_err("multiple manifests must not become nested installed packages");

        assert!(error
            .to_string()
            .contains("contains multiple omnidoc-package.toml files"));
    }

    #[test]
    fn validation_rejects_a_nested_manifest_added_after_installation() {
        let package = tempfile::tempdir().expect("package");
        write_plugin_package(package.path(), "return {}\n");
        fs::create_dir_all(package.path().join("nested")).expect("nested directory");
        fs::write(
            package.path().join("nested/omnidoc-package.toml"),
            "manifest_version = 2\n",
        )
        .expect("nested manifest");

        let inspection = inspect_package(
            &package.path().join(super::PACKAGE_MANIFEST_FILE),
            PackageScope::User,
            Some(PackageKind::Plugin),
        );
        assert!(!inspection.valid);
        assert!(inspection
            .errors
            .iter()
            .any(|error| error.contains("exactly one root omnidoc-package.toml")));
    }

    #[test]
    fn discovery_rejects_packages_outside_the_canonical_layout() {
        let workspace = tempfile::tempdir().expect("workspace");
        let store = workspace.path().join("store");
        let misplaced = store.join("plugins/unrelated/location/1.2.0");
        write_plugin_package(&misplaced, "return {}\n");
        let config = MergedConfig {
            extension_path: Some(store.to_string_lossy().to_string()),
            ..Default::default()
        };

        let packages = discover_packages(PackageKind::Plugin, None, &config)
            .expect("discover misplaced package");

        assert_eq!(packages.len(), 1);
        assert!(!packages[0].valid);
        assert!(packages[0]
            .errors
            .iter()
            .any(|error| error.contains("canonical plugins/ID/VERSION path")));
    }

    #[test]
    fn uninstall_removes_a_package_with_missing_declared_resources() {
        let workspace = tempfile::tempdir().expect("workspace");
        let source = workspace.path().join("source");
        let store = workspace.path().join("store");
        write_plugin_package(&source, "return {}\n");
        let config = MergedConfig {
            extension_path: Some(store.to_string_lossy().to_string()),
            ..Default::default()
        };
        let source_text = source.to_string_lossy().to_string();

        let installed = install_package(InstallPackageRequest {
            expected_kind: PackageKind::Plugin,
            source: &source_text,
            expected_sha256: None,
            project_root: None,
            config: &config,
            replace: false,
        })
        .expect("install package");
        let installed_root = std::path::PathBuf::from(&installed.destination);
        fs::remove_file(installed_root.join("filters/main.lua")).expect("remove declared filter");

        let discovered =
            discover_packages(PackageKind::Plugin, None, &config).expect("package discovery");
        assert_eq!(discovered.len(), 1);
        assert!(!discovered[0].valid);
        assert!(discovered[0]
            .errors
            .iter()
            .any(|error| error.contains("missing filter resource")));

        let removed = uninstall_package(PackageKind::Plugin, "acme/check@=1.2.0", None, &config)
            .expect("uninstall invalid package");
        assert!(removed.removed);
        assert!(!installed_root.exists());
    }

    #[test]
    fn uninstall_removes_a_package_with_an_unparseable_manifest() {
        let workspace = tempfile::tempdir().expect("workspace");
        let source = workspace.path().join("source");
        let store = workspace.path().join("store");
        write_plugin_package(&source, "return {}\n");
        let config = MergedConfig {
            extension_path: Some(store.to_string_lossy().to_string()),
            ..Default::default()
        };
        let source_text = source.to_string_lossy().to_string();

        let installed = install_package(InstallPackageRequest {
            expected_kind: PackageKind::Plugin,
            source: &source_text,
            expected_sha256: None,
            project_root: None,
            config: &config,
            replace: false,
        })
        .expect("install package");
        let installed_root = std::path::PathBuf::from(&installed.destination);
        fs::write(
            installed_root.join(super::PACKAGE_MANIFEST_FILE),
            "manifest_version = [\n",
        )
        .expect("corrupt manifest");

        let discovered =
            discover_packages(PackageKind::Plugin, None, &config).expect("package discovery");
        assert_eq!(discovered.len(), 1);
        assert!(!discovered[0].valid);
        assert!(discovered[0].manifest.is_none());

        let removed = uninstall_package(PackageKind::Plugin, "acme/check@=1.2.0", None, &config)
            .expect("uninstall malformed package");
        assert!(removed.removed);
        assert!(!installed_root.exists());
    }

    #[test]
    fn uninstall_removes_a_managed_package_with_a_missing_manifest() {
        let workspace = tempfile::tempdir().expect("workspace");
        let source = workspace.path().join("source");
        let store = workspace.path().join("store");
        write_plugin_package(&source, "return {}\n");
        let config = MergedConfig {
            extension_path: Some(store.to_string_lossy().to_string()),
            ..Default::default()
        };
        let source_text = source.to_string_lossy().to_string();

        let installed = install_package(InstallPackageRequest {
            expected_kind: PackageKind::Plugin,
            source: &source_text,
            expected_sha256: None,
            project_root: None,
            config: &config,
            replace: false,
        })
        .expect("install package");
        let installed_root = std::path::PathBuf::from(&installed.destination);
        fs::remove_file(installed_root.join(super::PACKAGE_MANIFEST_FILE))
            .expect("remove manifest");

        let removed = uninstall_package(PackageKind::Plugin, "acme/check@=1.2.0", None, &config)
            .expect("uninstall package without manifest");
        assert!(removed.removed);
        assert!(!installed_root.exists());
    }

    #[test]
    fn store_read_lock_blocks_package_replacement() {
        let workspace = tempfile::tempdir().expect("workspace");
        let source = workspace.path().join("source");
        let store = workspace.path().join("store");
        write_plugin_package(&source, "return {}\n");
        let config = MergedConfig {
            extension_path: Some(store.to_string_lossy().to_string()),
            ..Default::default()
        };
        let source_text = source.to_string_lossy().to_string();

        let read_locks = super::acquire_extension_store_read_locks(None, &config, "read packages")
            .expect("extension read lock");
        let error = install_package(InstallPackageRequest {
            expected_kind: PackageKind::Plugin,
            source: &source_text,
            expected_sha256: None,
            project_root: None,
            config: &config,
            replace: false,
        })
        .expect_err("store mutation must not race a reader");
        assert!(error.to_string().contains("another OmniDoc process"));

        drop(read_locks);
        install_package(InstallPackageRequest {
            expected_kind: PackageKind::Plugin,
            source: &source_text,
            expected_sha256: None,
            project_root: None,
            config: &config,
            replace: false,
        })
        .expect("install after releasing read lock");
    }

    #[test]
    fn interrupted_replacement_is_detected_and_recovered_before_mutation() {
        let workspace = tempfile::tempdir().expect("workspace");
        let source = workspace.path().join("source");
        let store = workspace.path().join("store");
        write_plugin_package(&source, "return {}\n");
        let config = MergedConfig {
            extension_path: Some(store.to_string_lossy().to_string()),
            ..Default::default()
        };
        let source_text = source.to_string_lossy().to_string();

        let installed = install_package(InstallPackageRequest {
            expected_kind: PackageKind::Plugin,
            source: &source_text,
            expected_sha256: None,
            project_root: None,
            config: &config,
            replace: false,
        })
        .expect("install package");
        let installed_root = std::path::PathBuf::from(&installed.destination);
        let transaction = store.join(".transactions/interrupted");
        fs::create_dir_all(&transaction).expect("transaction directory");
        fs::rename(&installed_root, transaction.join("backup")).expect("move package to backup");
        fs::write(
            transaction.join(super::TRANSACTION_RECORD_FILE),
            format!(
                r#"{{
  "transaction_version": 1,
  "destination": "plugins/acme/check/1.2.0",
  "payload_digest": "{}"
}}"#,
                installed.digest
            ),
        )
        .expect("transaction record");

        let error = super::acquire_extension_store_read_locks(None, &config, "build project")
            .err()
            .expect("readers must reject interrupted replacement");
        assert!(error.to_string().contains("interrupted replacement"));

        let recovered = install_package(InstallPackageRequest {
            expected_kind: PackageKind::Plugin,
            source: &source_text,
            expected_sha256: None,
            project_root: None,
            config: &config,
            replace: false,
        })
        .expect("recover before idempotent install");
        assert!(!recovered.installed);
        assert!(installed_root.is_dir());
        assert!(!store.join(".transactions").exists());
    }

    #[test]
    fn interrupted_replacement_accepts_an_already_promoted_matching_payload() {
        let workspace = tempfile::tempdir().expect("workspace");
        let old_source = workspace.path().join("old-source");
        let new_source = workspace.path().join("new-source");
        let store = workspace.path().join("store");
        write_plugin_package(&old_source, "return { version = 'old' }\n");
        write_plugin_package(&new_source, "return { version = 'new' }\n");
        let config = MergedConfig {
            extension_path: Some(store.to_string_lossy().to_string()),
            ..Default::default()
        };
        let old_source_text = old_source.to_string_lossy().to_string();
        let new_source_text = new_source.to_string_lossy().to_string();
        let installed = install_package(InstallPackageRequest {
            expected_kind: PackageKind::Plugin,
            source: &old_source_text,
            expected_sha256: None,
            project_root: None,
            config: &config,
            replace: false,
        })
        .expect("install old package");
        let installed_root = std::path::PathBuf::from(&installed.destination);
        let new_digest = directory_digest(&new_source).expect("new payload digest");
        let transaction = store.join(".transactions/promoted");
        fs::create_dir_all(&transaction).expect("transaction directory");
        fs::rename(&installed_root, transaction.join("backup")).expect("backup old package");
        super::copy_package_tree(&new_source, &installed_root).expect("promote new package");
        fs::write(
            transaction.join(super::TRANSACTION_RECORD_FILE),
            format!(
                r#"{{
  "transaction_version": 1,
  "destination": "plugins/acme/check/1.2.0",
  "payload_digest": "{}"
}}"#,
                new_digest
            ),
        )
        .expect("transaction record");

        let recovered = install_package(InstallPackageRequest {
            expected_kind: PackageKind::Plugin,
            source: &new_source_text,
            expected_sha256: None,
            project_root: None,
            config: &config,
            replace: false,
        })
        .expect("accept promoted package during recovery");

        assert!(!recovered.installed);
        assert_eq!(recovered.digest, new_digest);
        assert!(fs::read_to_string(installed_root.join("filters/main.lua"))
            .expect("promoted filter")
            .contains("version = 'new'"));
        assert!(!store.join(".transactions").exists());
    }

    #[test]
    fn interrupted_replacement_preserves_backup_when_promoted_payload_conflicts() {
        let workspace = tempfile::tempdir().expect("workspace");
        let old_source = workspace.path().join("old-source");
        let expected_source = workspace.path().join("expected-source");
        let conflicting_source = workspace.path().join("conflicting-source");
        let store = workspace.path().join("store");
        write_plugin_package(&old_source, "return { version = 'old' }\n");
        write_plugin_package(&expected_source, "return { version = 'expected' }\n");
        write_plugin_package(&conflicting_source, "return { version = 'conflicting' }\n");
        let config = MergedConfig {
            extension_path: Some(store.to_string_lossy().to_string()),
            ..Default::default()
        };
        let old_source_text = old_source.to_string_lossy().to_string();
        let expected_source_text = expected_source.to_string_lossy().to_string();
        let installed = install_package(InstallPackageRequest {
            expected_kind: PackageKind::Plugin,
            source: &old_source_text,
            expected_sha256: None,
            project_root: None,
            config: &config,
            replace: false,
        })
        .expect("install old package");
        let installed_root = std::path::PathBuf::from(&installed.destination);
        let expected_digest = directory_digest(&expected_source).expect("expected digest");
        let conflicting_digest = directory_digest(&conflicting_source).expect("conflicting digest");
        let transaction = store.join(".transactions/conflict");
        let backup = transaction.join("backup");
        fs::create_dir_all(&transaction).expect("transaction directory");
        fs::rename(&installed_root, &backup).expect("backup old package");
        super::copy_package_tree(&conflicting_source, &installed_root)
            .expect("promote conflicting package");
        fs::write(
            transaction.join(super::TRANSACTION_RECORD_FILE),
            format!(
                r#"{{
  "transaction_version": 1,
  "destination": "plugins/acme/check/1.2.0",
  "payload_digest": "{}"
}}"#,
                expected_digest
            ),
        )
        .expect("transaction record");

        let error = install_package(InstallPackageRequest {
            expected_kind: PackageKind::Plugin,
            source: &expected_source_text,
            expected_sha256: None,
            project_root: None,
            config: &config,
            replace: true,
        })
        .expect_err("conflicting promoted package must require manual recovery");

        assert!(error.to_string().contains("backup retained"));
        assert!(error.to_string().contains(&conflicting_digest));
        assert!(error.to_string().contains(&expected_digest));
        assert!(backup.is_dir());
        assert!(installed_root.is_dir());
        assert!(transaction.join(super::TRANSACTION_RECORD_FILE).is_file());
    }

    #[test]
    fn rejects_archive_path_traversal() {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file("../escape.lua", SimpleFileOptions::default())
            .expect("unsafe zip entry");
        writer.write_all(b"return {}\n").expect("zip content");
        let bytes = writer.finish().expect("zip finish").into_inner();
        let output = tempfile::tempdir().expect("output");

        let error = extract_zip(&bytes, output.path()).expect_err("path traversal must fail");
        assert!(error.to_string().contains("unsafe ZIP path"));
        assert!(!output.path().parent().unwrap().join("escape.lua").exists());
    }

    #[test]
    fn rejects_non_portable_and_case_colliding_archive_paths() {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file("Assets/one.txt", SimpleFileOptions::default())
            .expect("first zip entry");
        writer.write_all(b"one\n").expect("first content");
        writer
            .start_file("assets/two.txt", SimpleFileOptions::default())
            .expect("second zip entry");
        writer.write_all(b"two\n").expect("second content");
        let bytes = writer.finish().expect("zip finish").into_inner();
        let output = tempfile::tempdir().expect("output");
        let error = extract_zip(&bytes, output.path()).expect_err("case collision must fail");
        assert!(error.to_string().contains("case-insensitive filesystems"));

        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file(r"filters\main.lua", SimpleFileOptions::default())
            .expect("backslash zip entry");
        writer.write_all(b"return {}\n").expect("filter content");
        let bytes = writer.finish().expect("zip finish").into_inner();
        let output = tempfile::tempdir().expect("output");
        let error = extract_zip(&bytes, output.path()).expect_err("backslash path must fail");
        assert!(error.to_string().contains("unsafe ZIP path"));
    }

    #[test]
    fn package_digest_is_order_independent_and_tracks_nested_metadata_directories() {
        let package = tempfile::tempdir().expect("package");
        fs::create_dir_all(package.path().join(".git")).expect("metadata directory");
        let first = package.path().join("a.txt");
        let nested = package.path().join(".git/runtime.lua");
        fs::write(&first, "alpha\n").expect("first file");
        fs::write(&nested, "return 1\n").expect("nested file");

        let forward =
            digest_files(package.path(), &[first.clone(), nested.clone()]).expect("forward digest");
        let reverse =
            digest_files(package.path(), &[nested.clone(), first.clone()]).expect("reverse digest");
        assert_eq!(forward, reverse);

        let before = directory_digest(package.path()).expect("initial directory digest");
        fs::write(&nested, "return 2\n").expect("updated nested file");
        let after = directory_digest(package.path()).expect("updated directory digest");
        assert_ne!(before, after);

        let before_empty_directory = after;
        fs::create_dir_all(package.path().join("runtime/empty")).expect("empty directory");
        let after_empty_directory =
            directory_digest(package.path()).expect("digest with empty directory");
        assert_ne!(before_empty_directory, after_empty_directory);

        fs::write(
            package.path().join(super::INSTALL_RECEIPT_FILE),
            "internal receipt\n",
        )
        .expect("install receipt");
        assert_eq!(
            directory_digest(package.path()).expect("digest with receipt"),
            after_empty_directory
        );
    }

    #[cfg(unix)]
    #[test]
    fn enforces_declared_pandoc_compatibility() {
        use std::os::unix::fs::PermissionsExt;

        let workspace = tempfile::tempdir().expect("workspace");
        let pandoc = workspace.path().join("pandoc");
        fs::write(&pandoc, "#!/bin/sh\necho 'pandoc 2.19.2'\n").expect("fake pandoc");
        let mut permissions = fs::metadata(&pandoc).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&pandoc, permissions).expect("permissions");
        let mut config = MergedConfig::default();
        config.tool_paths.insert(
            "pandoc".to_string(),
            Some(pandoc.to_string_lossy().to_string()),
        );
        let package = ResolvedPackageIdentity {
            kind: PackageKind::Plugin,
            scope: PackageScope::User,
            id: "acme/check".to_string(),
            version: "1.0.0".to_string(),
            source: "test".to_string(),
            digest: "sha256:test".to_string(),
            root: workspace.path().to_path_buf(),
            tracked_files: Vec::new(),
            compatible_pandoc: Some(">=3,<4".to_string()),
        };

        let error = ensure_pandoc_compatible(std::slice::from_ref(&package), &config)
            .expect_err("Pandoc 2 must be rejected");
        assert!(error.to_string().contains("requires Pandoc"));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symbolic_links_in_local_packages() {
        use std::os::unix::fs::symlink;

        let workspace = tempfile::tempdir().expect("workspace");
        let source = workspace.path().join("source");
        let store = workspace.path().join("store");
        write_plugin_package(&source, "return {}\n");
        fs::write(workspace.path().join("outside.lua"), "return {}\n").expect("outside file");
        symlink(
            workspace.path().join("outside.lua"),
            source.join("filters/linked.lua"),
        )
        .expect("package symlink");
        let config = MergedConfig {
            extension_path: Some(store.to_string_lossy().to_string()),
            ..Default::default()
        };
        let source_text = source.to_string_lossy().to_string();

        let error = install_package(InstallPackageRequest {
            expected_kind: PackageKind::Plugin,
            source: &source_text,
            expected_sha256: None,
            project_root: None,
            config: &config,
            replace: false,
        })
        .expect_err("symlink package must fail");
        assert!(error.to_string().contains("symbolic links are not allowed"));
    }

    #[cfg(unix)]
    #[test]
    fn refuses_package_destinations_through_store_symlinks() {
        use std::os::unix::fs::symlink;

        let workspace = tempfile::tempdir().expect("workspace");
        let source = workspace.path().join("source");
        let store = workspace.path().join("store");
        let outside = workspace.path().join("outside");
        write_plugin_package(&source, "return {}\n");
        fs::create_dir_all(store.join("plugins")).expect("plugin store");
        fs::create_dir_all(&outside).expect("outside directory");
        symlink(&outside, store.join("plugins/acme")).expect("namespace symlink");
        let config = MergedConfig {
            extension_path: Some(store.to_string_lossy().to_string()),
            ..Default::default()
        };
        let source_text = source.to_string_lossy().to_string();

        let error = install_package(InstallPackageRequest {
            expected_kind: PackageKind::Plugin,
            source: &source_text,
            expected_sha256: None,
            project_root: None,
            config: &config,
            replace: false,
        })
        .expect_err("store namespace symlink must fail");
        assert!(error
            .to_string()
            .contains("must not traverse a symbolic link"));
        assert!(!outside.join("1.2.0").exists());
    }
}
