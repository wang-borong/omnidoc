use crate::config::global::GlobalConfig;
use crate::constants::config as config_consts;
use crate::constants::git_refs;
use crate::doc::templates::get_latexmkrc_template;
use crate::error::{OmniDocError, Result};
use crate::git::{git_checkout_revision, git_clone};
use crate::utils::fs;
use console::style;
use dirs::{config_local_dir, data_local_dir};
use flate2::read::GzDecoder;
use git2::{Repository, StatusOptions};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Read};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tar::Archive;
use walkdir::WalkDir;

const LIBRARY_RELEASE_CONTRACT: &str = include_str!("../../../release/omnidoc-libs.toml");
const INSTALLED_RELEASE_FILE: &str = ".omnidoc-release.toml";
const MAX_RELEASE_ARCHIVE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_RELEASE_CHECKSUM_BYTES: u64 = 64 * 1024;
const MAX_RELEASE_EXTRACTED_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Debug, Deserialize)]
struct LibraryManifest {
    manifest_version: u32,
    version: String,
    compatible_omnidoc: String,
    compatible_pandoc: String,
    checksum_algorithm: String,
    checksum_file: String,
    payload_roots: Vec<String>,
    required_resources: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct LibraryReleaseContract {
    contract_version: u32,
    omnidoc_version: String,
    library: ReleasedLibrary,
}

#[derive(Debug, Deserialize)]
struct ReleasedLibrary {
    version: String,
    revision: String,
    archive_url: String,
    checksum_algorithm: String,
    checksum_url: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct InstalledRelease {
    contract_version: u32,
    version: String,
    revision: String,
    archive_url: String,
    archive_digest: String,
}

#[derive(Debug, Serialize)]
struct LibraryStatus {
    path: String,
    exists: bool,
    source: Option<String>,
    manifest_valid: bool,
    integrity_verified: bool,
    version: Option<String>,
    revision: Option<String>,
    archive_digest: Option<String>,
    requested_revision: Option<String>,
    revision_matches: Option<bool>,
    dirty: Option<bool>,
    compatible_omnidoc: Option<String>,
    omnidoc_compatible: Option<bool>,
    compatible_pandoc: Option<String>,
    pandoc_version: Option<String>,
    pandoc_compatible: Option<bool>,
    checked_files: usize,
    errors: Vec<String>,
}

/// Handle the `lib` / `libs` command.
pub fn handle_lib(
    install: bool,
    update: bool,
    status: bool,
    verify: bool,
    json: bool,
    revision: Option<String>,
    release: bool,
) -> Result<()> {
    let global_config = GlobalConfig::load()?;
    let library_path = configured_library_path(&global_config)?;
    let release_source =
        release || (revision.is_none() && library_path.join(INSTALLED_RELEASE_FILE).is_file());
    let requested_revision = if release_source {
        None
    } else {
        revision.or_else(|| configured_library_revision(&global_config))
    };
    let requested_install = install || (!update && !status && !verify);

    if requested_install {
        if release_source {
            install_release_library(&library_path, false)?;
        } else {
            let lib_url = configured_library_url(&global_config);
            install_verified_library(
                &lib_url,
                &library_path,
                requested_revision.as_deref(),
                false,
                json,
            )?;
        }
        ensure_latexmkrc()?;
        let result = inspect_library(&library_path, requested_revision.as_deref());
        print_library_status(&result, json)?;
        ensure_verified(&result)?;
        if !json {
            println!(
                "{} {} '{}'",
                style("✔").green().bold(),
                style("OmniDoc library installed in").green().bold(),
                library_path.display()
            );
        }
        return Ok(());
    }

    if update {
        if release_source {
            install_release_library(&library_path, true)?;
        } else {
            ensure_clean_repository(&library_path)?;
            let lib_url = installed_library_url(&library_path)
                .unwrap_or_else(|| configured_library_url(&global_config));
            install_verified_library(
                &lib_url,
                &library_path,
                requested_revision.as_deref(),
                true,
                json,
            )?;
        }
        ensure_latexmkrc()?;
        let result = inspect_library(&library_path, requested_revision.as_deref());
        print_library_status(&result, json)?;
        ensure_verified(&result)?;
        if !json {
            println!(
                "{} {} '{}'",
                style("✔").green().bold(),
                style("OmniDoc library updated in").green().bold(),
                library_path.display()
            );
        }
        return Ok(());
    }

    let result = inspect_library(&library_path, requested_revision.as_deref());
    print_library_status(&result, json)?;
    if verify {
        ensure_verified(&result)?;
    }
    Ok(())
}

fn install_release_library(library_path: &Path, replace_existing: bool) -> Result<()> {
    let contract = read_release_contract()?;
    let archive = download_release_file(&contract.library.archive_url, MAX_RELEASE_ARCHIVE_BYTES)?;
    let checksum =
        download_release_file(&contract.library.checksum_url, MAX_RELEASE_CHECKSUM_BYTES)?;
    verify_release_archive_checksum(&archive, &checksum, &contract.library.archive_url)?;
    let archive_digest = format!("sha256:{:x}", Sha256::digest(&archive));

    let parent = library_path.parent().ok_or_else(|| {
        OmniDocError::Other(format!(
            "library path has no parent directory: {}",
            library_path.display()
        ))
    })?;
    fs::create_dir_all(parent)?;
    let container = sibling_transaction_path(library_path, "archive");
    let staging = sibling_transaction_path(library_path, "staging");
    let prepared = (|| -> Result<LibraryStatus> {
        extract_release_archive(&archive, &container, &contract.library.version)?;
        let extracted = container.join(format!("omnidoc-libs-v{}", contract.library.version));
        fs::rename(&extracted, &staging)?;
        write_installed_release(
            &staging,
            &InstalledRelease {
                contract_version: contract.contract_version,
                version: contract.library.version.clone(),
                revision: contract.library.revision.clone(),
                archive_url: contract.library.archive_url.clone(),
                archive_digest: archive_digest.clone(),
            },
        )?;
        let status = inspect_library(&staging, None);
        ensure_verified(&status)?;
        if status.version.as_deref() != Some(contract.library.version.as_str()) {
            return Err(OmniDocError::Other(format!(
                "release archive library version {:?} does not match contract {}",
                status.version, contract.library.version
            )));
        }
        Ok(status)
    })();
    cleanup_transaction_path(&container);
    match prepared {
        Ok(_) => {}
        Err(error) => {
            cleanup_transaction_path(&staging);
            return Err(error);
        }
    }

    if !replace_existing {
        if library_path.exists() {
            cleanup_transaction_path(&staging);
            return Err(OmniDocError::Other(format!(
                "OmniDoc library already exists: {}",
                library_path.display()
            )));
        }
        if let Err(error) = fs::rename(&staging, library_path) {
            cleanup_transaction_path(&staging);
            return Err(error);
        }
    } else {
        replace_library_transactionally(library_path, &staging)?;
    }
    Ok(())
}

fn write_installed_release(root: &Path, release: &InstalledRelease) -> Result<()> {
    let content = toml::to_string_pretty(release).map_err(|error| {
        OmniDocError::Other(format!("cannot serialize release metadata: {error}"))
    })?;
    fs::write(root.join(INSTALLED_RELEASE_FILE), content.as_bytes())
}

fn read_release_contract() -> Result<LibraryReleaseContract> {
    let contract =
        toml::from_str::<LibraryReleaseContract>(LIBRARY_RELEASE_CONTRACT).map_err(|error| {
            OmniDocError::Other(format!("invalid library release contract: {error}"))
        })?;
    if contract.contract_version != 1 {
        return Err(OmniDocError::Other(format!(
            "unsupported library release contract version {}",
            contract.contract_version
        )));
    }
    if contract.omnidoc_version != env!("CARGO_PKG_VERSION") {
        return Err(OmniDocError::Other(format!(
            "library release contract targets OmniDoc {}, running {}",
            contract.omnidoc_version,
            env!("CARGO_PKG_VERSION")
        )));
    }
    if contract.library.checksum_algorithm != "sha256" {
        return Err(OmniDocError::Other(format!(
            "unsupported release checksum algorithm {}",
            contract.library.checksum_algorithm
        )));
    }
    Version::parse(&contract.library.version).map_err(|error| {
        OmniDocError::Other(format!(
            "invalid library release version {}: {error}",
            contract.library.version
        ))
    })?;
    let expected_revision = format!("v{}", contract.library.version);
    if contract.library.revision != expected_revision {
        return Err(OmniDocError::Other(format!(
            "library release revision {} does not match version {}",
            contract.library.revision, contract.library.version
        )));
    }
    Ok(contract)
}

fn download_release_file(url: &str, max_bytes: u64) -> Result<Vec<u8>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|error| OmniDocError::Other(format!("cannot create HTTP client: {error}")))?;
    let response = client
        .get(url)
        .send()
        .map_err(|error| OmniDocError::Other(format!("cannot download {url}: {error}")))?;
    let status = response.status();
    if !status.is_success() {
        return Err(OmniDocError::HttpError {
            status: status.as_u16(),
            url: url.to_string(),
        });
    }
    if response
        .content_length()
        .is_some_and(|length| length > max_bytes)
    {
        return Err(OmniDocError::Other(format!(
            "download exceeds size limit ({max_bytes} bytes): {url}"
        )));
    }
    let mut bytes = Vec::new();
    response
        .take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| OmniDocError::Other(format!("cannot read {url}: {error}")))?;
    if bytes.len() as u64 > max_bytes {
        return Err(OmniDocError::Other(format!(
            "download exceeds size limit ({max_bytes} bytes): {url}"
        )));
    }
    Ok(bytes)
}

fn verify_release_archive_checksum(
    archive: &[u8],
    checksum: &[u8],
    archive_url: &str,
) -> Result<()> {
    let checksum = std::str::from_utf8(checksum).map_err(|error| {
        OmniDocError::Other(format!("invalid release checksum encoding: {error}"))
    })?;
    let line = checksum
        .lines()
        .find(|line| !line.trim().is_empty())
        .ok_or_else(|| OmniDocError::Other("empty release checksum file".to_string()))?;
    let mut fields = line.split_whitespace();
    let expected = fields
        .next()
        .filter(|value| {
            value.len() == 64
                && value.chars().all(|character| {
                    character.is_ascii_hexdigit() && !character.is_ascii_uppercase()
                })
        })
        .ok_or_else(|| OmniDocError::Other("invalid release SHA-256 checksum".to_string()))?;
    let expected_name = archive_url
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| OmniDocError::Other("invalid release archive URL".to_string()))?;
    let checksum_name = fields
        .next()
        .map(|name| name.trim_start_matches('*'))
        .ok_or_else(|| OmniDocError::Other("release checksum is missing a filename".to_string()))?;
    if checksum_name != expected_name {
        return Err(OmniDocError::Other(format!(
            "release checksum filename {checksum_name} does not match {expected_name}"
        )));
    }
    let actual = format!("{:x}", Sha256::digest(archive));
    if actual != expected {
        return Err(OmniDocError::Other(format!(
            "release archive checksum mismatch for {archive_url}: expected {expected}, got {actual}"
        )));
    }
    Ok(())
}

fn extract_release_archive(archive: &[u8], destination: &Path, version: &str) -> Result<()> {
    fs::create_dir_all(destination)?;
    let expected_root = PathBuf::from(format!("omnidoc-libs-v{version}"));
    let decoder = GzDecoder::new(Cursor::new(archive));
    let mut archive = Archive::new(decoder);
    let mut extracted_bytes = 0_u64;
    let mut extracted_paths = BTreeSet::new();
    for entry in archive
        .entries()
        .map_err(|error| OmniDocError::Other(format!("cannot read release archive: {error}")))?
    {
        let mut entry = entry
            .map_err(|error| OmniDocError::Other(format!("cannot read archive entry: {error}")))?;
        let path = entry
            .path()
            .map_err(|error| OmniDocError::Other(format!("invalid archive path: {error}")))?
            .into_owned();
        if !path.starts_with(&expected_root)
            || path
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(OmniDocError::Other(format!(
                "unsafe or unexpected release archive path: {}",
                path.display()
            )));
        }
        if !extracted_paths.insert(path.clone()) {
            return Err(OmniDocError::Other(format!(
                "duplicate release archive path: {}",
                path.display()
            )));
        }
        let entry_type = entry.header().entry_type();
        if !entry_type.is_file() && !entry_type.is_dir() {
            return Err(OmniDocError::Other(format!(
                "unsupported release archive entry type: {}",
                path.display()
            )));
        }
        extracted_bytes = extracted_bytes
            .checked_add(entry.header().size().unwrap_or(0))
            .ok_or_else(|| OmniDocError::Other("release archive size overflow".to_string()))?;
        if extracted_bytes > MAX_RELEASE_EXTRACTED_BYTES {
            return Err(OmniDocError::Other(format!(
                "release archive expands beyond size limit ({} bytes)",
                MAX_RELEASE_EXTRACTED_BYTES
            )));
        }
        let unpacked = entry.unpack_in(destination).map_err(|error| {
            OmniDocError::Other(format!("cannot extract {}: {error}", path.display()))
        })?;
        if !unpacked {
            return Err(OmniDocError::Other(format!(
                "release archive entry escapes destination: {}",
                path.display()
            )));
        }
    }
    let root = destination.join(expected_root);
    if !root.is_dir() {
        return Err(OmniDocError::Other(
            "release archive is missing its versioned root directory".to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn configured_library_path(global_config: &GlobalConfig) -> Result<PathBuf> {
    if let Some(path) = global_config
        .get_config()
        .and_then(|config| config.lib.lib.as_ref())
        .and_then(|library| library.path.as_ref())
    {
        return Ok(PathBuf::from(path));
    }
    data_local_dir()
        .map(|path| path.join("omnidoc"))
        .ok_or_else(|| OmniDocError::Other("Local data directory not found".to_string()))
}

pub(crate) fn library_diagnostic(path: &Path) -> (bool, String) {
    let status = inspect_library(path, None);
    let ok = status.exists
        && status.manifest_valid
        && status.integrity_verified
        && status.errors.is_empty()
        && status.omnidoc_compatible != Some(false)
        && status.pandoc_compatible != Some(false);
    let detail = if ok {
        format!(
            "version {}; source {}; revision {}; {} payload files verified",
            status.version.as_deref().unwrap_or("unknown"),
            status.source.as_deref().unwrap_or("local"),
            status.revision.as_deref().unwrap_or("unknown"),
            status.checked_files
        )
    } else if status.errors.is_empty() {
        format!("library verification failed at {}", path.display())
    } else {
        status.errors.join("; ")
    };
    (ok, detail)
}

fn configured_library_url(global_config: &GlobalConfig) -> String {
    global_config
        .get_config()
        .and_then(|config| config.lib.lib.as_ref())
        .and_then(|library| library.url.clone())
        .unwrap_or_else(|| config_consts::DEFAULT_LIB_URL.to_string())
}

fn configured_library_revision(global_config: &GlobalConfig) -> Option<String> {
    global_config
        .get_config()
        .and_then(|config| config.lib.lib.as_ref())
        .and_then(|library| library.revision.clone())
}

fn installed_library_url(path: &Path) -> Option<String> {
    let repository = Repository::open(path).ok()?;
    let remote = repository.find_remote(git_refs::ORIGIN).ok()?;
    remote.url().ok().map(str::to_string)
}

fn install_verified_library(
    url: &str,
    library_path: &Path,
    requested_revision: Option<&str>,
    replace_existing: bool,
    json: bool,
) -> Result<()> {
    let parent = library_path.parent().ok_or_else(|| {
        OmniDocError::Other(format!(
            "library path has no parent directory: {}",
            library_path.display()
        ))
    })?;
    fs::create_dir_all(parent)?;
    let staging = sibling_transaction_path(library_path, "staging");
    let prepared = (|| -> Result<LibraryStatus> {
        git_clone(url, &staging, true).map_err(OmniDocError::Git)?;
        let checkout_revision =
            requested_revision.or_else(|| replace_existing.then_some(git_refs::MAIN_BRANCH));
        if let Some(revision) = checkout_revision {
            git_checkout_revision(&staging, revision).map_err(OmniDocError::Git)?;
        }
        Ok(inspect_library(&staging, requested_revision))
    })();
    let staged_status = match prepared {
        Ok(status) => status,
        Err(error) => {
            cleanup_transaction_path(&staging);
            return Err(error);
        }
    };
    if let Err(error) = ensure_verified(&staged_status) {
        cleanup_transaction_path(&staging);
        print_library_status(&staged_status, json)?;
        return Err(error);
    }

    if !replace_existing {
        if library_path.exists() {
            cleanup_transaction_path(&staging);
            return Err(OmniDocError::Other(format!(
                "OmniDoc library already exists: {}",
                library_path.display()
            )));
        }
        if let Err(error) = fs::rename(&staging, library_path) {
            cleanup_transaction_path(&staging);
            return Err(error);
        }
        return Ok(());
    }

    replace_library_transactionally(library_path, &staging)
}

fn replace_library_transactionally(library_path: &Path, staging: &Path) -> Result<()> {
    let backup = sibling_transaction_path(library_path, "backup");
    if let Err(error) = fs::rename(library_path, &backup) {
        cleanup_transaction_path(staging);
        return Err(error);
    }
    if let Err(error) = fs::rename(staging, library_path) {
        let restore = fs::rename(&backup, library_path);
        cleanup_transaction_path(staging);
        if let Err(restore_error) = restore {
            return Err(OmniDocError::Other(format!(
                "failed to promote verified library ({error}); failed to restore previous library ({restore_error})"
            )));
        }
        return Err(error);
    }
    if let Err(error) = fs::remove_dir_all(&backup) {
        eprintln!(
            "Warning: verified library installed but old backup could not be removed: {} ({})",
            backup.display(),
            error
        );
    }
    Ok(())
}

fn sibling_transaction_path(path: &Path, kind: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("omnidoc");
    path.with_file_name(format!(".{name}.{kind}.{}.{nonce}", std::process::id()))
}

fn cleanup_transaction_path(path: &Path) {
    if path.exists() {
        let _ = fs::remove_dir_all(path);
    }
}

fn ensure_clean_repository(path: &Path) -> Result<()> {
    let repository = Repository::open(path).map_err(OmniDocError::Git)?;
    let mut options = StatusOptions::new();
    options.include_untracked(true).recurse_untracked_dirs(true);
    let dirty = repository
        .statuses(Some(&mut options))
        .map_err(OmniDocError::Git)?
        .iter()
        .next()
        .is_some();
    if dirty {
        return Err(OmniDocError::Other(format!(
            "refusing to update dirty OmniDoc library: {}",
            path.display()
        )));
    }
    Ok(())
}

fn ensure_latexmkrc() -> Result<()> {
    let mut latexmkrc = config_local_dir().ok_or_else(|| {
        OmniDocError::Other("Local configuration directory not found".to_string())
    })?;
    latexmkrc.push("latexmk");
    if !fs::exists(&latexmkrc) {
        fs::create_dir_all(&latexmkrc)?;
    }
    latexmkrc.push("latexmkrc");
    if !fs::exists(&latexmkrc) {
        fs::write(&latexmkrc, get_latexmkrc_template().as_bytes())?;
    }
    Ok(())
}

fn inspect_library(path: &Path, requested_revision: Option<&str>) -> LibraryStatus {
    let mut status = LibraryStatus {
        path: path.display().to_string(),
        exists: path.is_dir(),
        source: None,
        manifest_valid: false,
        integrity_verified: false,
        version: None,
        revision: None,
        archive_digest: None,
        requested_revision: requested_revision.map(str::to_string),
        revision_matches: None,
        dirty: None,
        compatible_omnidoc: None,
        omnidoc_compatible: None,
        compatible_pandoc: None,
        pandoc_version: None,
        pandoc_compatible: None,
        checked_files: 0,
        errors: Vec::new(),
    };
    if !status.exists {
        status
            .errors
            .push("library directory does not exist".to_string());
        return status;
    }

    if let Ok(repository) = Repository::open(path) {
        status.source = Some("git".to_string());
        status.revision = repository
            .head()
            .ok()
            .and_then(|head| head.target())
            .map(|oid| oid.to_string());
        let mut options = StatusOptions::new();
        options.include_untracked(true).recurse_untracked_dirs(true);
        status.dirty = repository
            .statuses(Some(&mut options))
            .ok()
            .map(|statuses| !statuses.is_empty());
        if let (Some(requested), Some(actual)) = (requested_revision, status.revision.as_deref()) {
            status.revision_matches = repository
                .revparse_single(requested)
                .ok()
                .and_then(|object| object.peel_to_commit().ok())
                .map(|commit| commit.id().to_string() == actual);
            if status.revision_matches == Some(false) {
                status.errors.push(format!(
                    "installed revision {} does not match requested revision {}",
                    actual, requested
                ));
            } else if status.revision_matches.is_none() {
                status
                    .errors
                    .push(format!("cannot resolve requested revision {}", requested));
            }
        }
    } else {
        let release_path = path.join(INSTALLED_RELEASE_FILE);
        if release_path.is_file() {
            match std::fs::read_to_string(&release_path)
                .map_err(|error| error.to_string())
                .and_then(|content| {
                    toml::from_str::<InstalledRelease>(&content).map_err(|error| error.to_string())
                }) {
                Ok(release) => {
                    let expected_revision = format!("v{}", release.version);
                    status.source = Some("release".to_string());
                    status.revision = Some(release.revision.clone());
                    status.archive_digest = Some(release.archive_digest.clone());
                    if release.contract_version != 1 {
                        status.errors.push(format!(
                            "unsupported installed release metadata version {}",
                            release.contract_version
                        ));
                    }
                    if release.revision != expected_revision {
                        status.errors.push(format!(
                            "installed release revision {} does not match version {}",
                            release.revision, release.version
                        ));
                    }
                    if !valid_sha256_digest(&release.archive_digest) {
                        status
                            .errors
                            .push("invalid installed release archive digest".to_string());
                    }
                }
                Err(error) => {
                    status
                        .errors
                        .push(format!("invalid {}: {}", release_path.display(), error))
                }
            }
        }
    }

    let manifest = match read_manifest(path) {
        Ok(manifest) => manifest,
        Err(error) => {
            status.errors.push(error);
            return status;
        }
    };
    status.manifest_valid = true;
    status.version = Some(manifest.version.clone());
    if let Ok(content) = std::fs::read_to_string(path.join(INSTALLED_RELEASE_FILE)) {
        if let Ok(release) = toml::from_str::<InstalledRelease>(&content) {
            if release.version != manifest.version {
                status.errors.push(format!(
                    "installed release metadata version {} does not match manifest {}",
                    release.version, manifest.version
                ));
            }
        }
    }
    status.compatible_omnidoc = Some(manifest.compatible_omnidoc.clone());
    status.compatible_pandoc = Some(manifest.compatible_pandoc.clone());
    status.omnidoc_compatible =
        version_matches(&manifest.compatible_omnidoc, env!("CARGO_PKG_VERSION"));
    status.pandoc_version = command_version("pandoc");
    status.pandoc_compatible = status
        .pandoc_version
        .as_deref()
        .and_then(|version| version_matches(&manifest.compatible_pandoc, version));

    match verify_checksums(path, &manifest) {
        Ok(checked_files) => {
            status.integrity_verified = true;
            status.checked_files = checked_files;
        }
        Err(errors) => status.errors.extend(errors),
    }
    status
}

fn read_manifest(root: &Path) -> std::result::Result<LibraryManifest, String> {
    let path = root.join("manifest.toml");
    let content = std::fs::read_to_string(&path)
        .map_err(|error| format!("cannot read {}: {}", path.display(), error))?;
    let manifest = toml::from_str::<LibraryManifest>(&content)
        .map_err(|error| format!("invalid manifest.toml: {}", error))?;
    if manifest.manifest_version != 1 {
        return Err(format!(
            "unsupported manifest_version {}",
            manifest.manifest_version
        ));
    }
    if manifest.checksum_algorithm != "sha256" {
        return Err(format!(
            "unsupported checksum algorithm {}",
            manifest.checksum_algorithm
        ));
    }
    Version::parse(&manifest.version)
        .map_err(|error| format!("invalid library version: {}", error))?;
    VersionReq::parse(&manifest.compatible_omnidoc)
        .map_err(|error| format!("invalid OmniDoc compatibility requirement: {}", error))?;
    VersionReq::parse(&manifest.compatible_pandoc)
        .map_err(|error| format!("invalid Pandoc compatibility requirement: {}", error))?;
    Ok(manifest)
}

fn verify_checksums(
    root: &Path,
    manifest: &LibraryManifest,
) -> std::result::Result<usize, Vec<String>> {
    let mut errors = Vec::new();
    for required in &manifest.required_resources {
        match safe_join(root, required) {
            Some(path) if path.is_file() => {}
            Some(_) => errors.push(format!("missing required resource: {}", required)),
            None => errors.push(format!("unsafe required resource path: {}", required)),
        }
    }

    let checksum_path = match safe_join(root, &manifest.checksum_file) {
        Some(path) => path,
        None => {
            errors.push("unsafe checksum_file path".to_string());
            return Err(errors);
        }
    };
    let expected = match read_checksum_file(&checksum_path) {
        Ok(checksums) => checksums,
        Err(error) => {
            errors.push(error);
            return Err(errors);
        }
    };
    let actual_files = match collect_payload_files(root, &manifest.payload_roots) {
        Ok(files) => files,
        Err(error) => {
            errors.push(error);
            return Err(errors);
        }
    };
    let expected_files = expected.keys().cloned().collect::<BTreeSet<_>>();
    for missing in actual_files.difference(&expected_files) {
        errors.push(format!("missing checksum entry: {}", missing));
    }
    for extra in expected_files.difference(&actual_files) {
        errors.push(format!("checksum references missing payload: {}", extra));
    }
    for relative in actual_files.intersection(&expected_files) {
        let Some(path) = safe_join(root, relative) else {
            errors.push(format!("unsafe checksum path: {}", relative));
            continue;
        };
        match sha256(&path) {
            Ok(actual) if expected.get(relative) == Some(&actual) => {}
            Ok(_) => errors.push(format!("checksum mismatch: {}", relative)),
            Err(error) => errors.push(error),
        }
    }
    if errors.is_empty() {
        Ok(actual_files.len())
    } else {
        Err(errors)
    }
}

fn collect_payload_files(
    root: &Path,
    payload_roots: &[String],
) -> std::result::Result<BTreeSet<String>, String> {
    let mut files = BTreeSet::new();
    for relative in payload_roots {
        let path = safe_join(root, relative)
            .ok_or_else(|| format!("unsafe payload root: {}", relative))?;
        if path.is_file() {
            files.insert(relative.clone());
            continue;
        }
        if !path.is_dir() {
            return Err(format!("missing payload root: {}", relative));
        }
        for entry in WalkDir::new(&path).follow_links(false) {
            let entry = entry.map_err(|error| error.to_string())?;
            if entry.file_type().is_symlink() {
                return Err(format!(
                    "symbolic link is not allowed: {}",
                    entry.path().display()
                ));
            }
            if entry.file_type().is_file() {
                files.insert(
                    entry
                        .path()
                        .strip_prefix(root)
                        .map_err(|error| error.to_string())?
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
    }
    Ok(files)
}

fn read_checksum_file(path: &Path) -> std::result::Result<BTreeMap<String, String>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {}", path.display(), error))?;
    let mut checksums = BTreeMap::new();
    for (index, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let Some((checksum, relative)) = line.split_once("  ") else {
            return Err(format!("invalid checksum line {}", index + 1));
        };
        if checksum.len() != 64
            || !checksum
                .chars()
                .all(|character| character.is_ascii_hexdigit() && !character.is_ascii_uppercase())
        {
            return Err(format!("invalid SHA-256 on line {}", index + 1));
        }
        if safe_relative_path(relative).is_none() {
            return Err(format!("unsafe checksum path on line {}", index + 1));
        }
        if checksums
            .insert(relative.to_string(), checksum.to_string())
            .is_some()
        {
            return Err(format!("duplicate checksum path: {}", relative));
        }
    }
    Ok(checksums)
}

fn sha256(path: &Path) -> std::result::Result<String, String> {
    let bytes = std::fs::read(path)
        .map_err(|error| format!("cannot read {}: {}", path.display(), error))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn safe_join(root: &Path, relative: &str) -> Option<PathBuf> {
    safe_relative_path(relative).map(|path| root.join(path))
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

fn version_matches(requirement: &str, version: &str) -> Option<bool> {
    let requirement = VersionReq::parse(requirement).ok()?;
    let version = version.split_whitespace().find_map(parse_version_token)?;
    Some(requirement.matches(&version))
}

fn valid_sha256_digest(digest: &str) -> bool {
    digest.strip_prefix("sha256:").is_some_and(|value| {
        value.len() == 64
            && value
                .chars()
                .all(|character| character.is_ascii_hexdigit() && !character.is_ascii_uppercase())
    })
}

fn parse_version_token(token: &str) -> Option<Version> {
    let token = token.trim_start_matches('v');
    if let Ok(version) = Version::parse(token) {
        return Some(version);
    }
    let numeric = token
        .chars()
        .take_while(|character| character.is_ascii_digit() || *character == '.')
        .collect::<String>();
    let components = numeric.split('.').count();
    match components {
        1 => Version::parse(&format!("{}.0.0", numeric)).ok(),
        2 => Version::parse(&format!("{}.0", numeric)).ok(),
        _ => None,
    }
}

fn command_version(program: &str) -> Option<String> {
    let output = Command::new(program).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
}

fn print_library_status(status: &LibraryStatus, json: bool) -> Result<()> {
    if json {
        let content = serde_json::to_string_pretty(status)
            .map_err(|error| OmniDocError::Other(error.to_string()))?;
        println!("{}", content);
        return Ok(());
    }
    println!("path: {}", status.path);
    println!("exists: {}", status.exists);
    println!("source: {}", status.source.as_deref().unwrap_or("local"));
    println!(
        "version: {}",
        status.version.as_deref().unwrap_or("unknown")
    );
    if let Some(digest) = status.archive_digest.as_deref() {
        println!("archive: {}", digest);
    }
    println!(
        "revision: {}{}",
        status.revision.as_deref().unwrap_or("unknown"),
        if status.dirty == Some(true) {
            " (dirty)"
        } else {
            ""
        }
    );
    if let Some(requested) = status.requested_revision.as_deref() {
        println!(
            "requested revision: {} ({})",
            requested,
            match status.revision_matches {
                Some(true) => "matched",
                Some(false) => "mismatched",
                None => "unresolved",
            }
        );
    }
    println!(
        "manifest: {}",
        if status.manifest_valid {
            "valid"
        } else {
            "invalid"
        }
    );
    println!(
        "integrity: {} ({} files)",
        if status.integrity_verified {
            "verified"
        } else {
            "failed"
        },
        status.checked_files
    );
    println!(
        "OmniDoc compatibility: {} ({})",
        compatibility_label(status.omnidoc_compatible),
        status.compatible_omnidoc.as_deref().unwrap_or("unknown")
    );
    println!(
        "Pandoc compatibility: {} ({}, installed {})",
        compatibility_label(status.pandoc_compatible),
        status.compatible_pandoc.as_deref().unwrap_or("unknown"),
        status.pandoc_version.as_deref().unwrap_or("not found")
    );
    for error in &status.errors {
        println!("error: {}", error);
    }
    Ok(())
}

fn compatibility_label(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "compatible",
        Some(false) => "incompatible",
        None => "unknown",
    }
}

fn ensure_verified(status: &LibraryStatus) -> Result<()> {
    if status.integrity_verified
        && status.manifest_valid
        && status.errors.is_empty()
        && status.omnidoc_compatible != Some(false)
        && status.pandoc_compatible != Some(false)
        && status.revision_matches != Some(false)
        && !(status.requested_revision.is_some() && status.revision_matches.is_none())
    {
        return Ok(());
    }
    Err(OmniDocError::Other(
        "OmniDoc library verification failed".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        extract_release_archive, git_refs, inspect_library, install_verified_library,
        read_manifest, safe_relative_path, verify_release_archive_checksum, version_matches,
        write_installed_release, InstalledRelease,
    };
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use git2::{Repository, RepositoryInitOptions, Signature};
    use sha2::{Digest, Sha256};
    use std::fs;
    use std::io::Cursor;
    use std::path::Path;
    use tar::{Builder, Header};
    use tempfile::TempDir;

    fn commit_all(repository: &Repository, message: &str) -> git2::Oid {
        let mut index = repository.index().expect("repository index");
        index
            .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
            .expect("add files");
        let tree_id = index.write_tree().expect("write tree");
        let tree = repository.find_tree(tree_id).expect("find tree");
        let signature =
            Signature::now("OmniDoc Test", "omnidoc@example.invalid").expect("signature");
        let parent = repository
            .head()
            .ok()
            .and_then(|head| head.peel_to_commit().ok());
        let parents = parent.iter().collect::<Vec<_>>();
        repository
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &parents,
            )
            .expect("commit")
    }

    fn write_library_contract(root: &Path, payload: &[u8], checksum_payload: &[u8]) {
        fs::create_dir_all(root.join("payload")).expect("payload directory");
        fs::write(root.join("payload/resource.txt"), payload).expect("payload");
        fs::write(
            root.join("manifest.toml"),
            r#"manifest_version = 1
version = "1.0.0"
compatible_omnidoc = ">=1.3.0,<2.0.0"
compatible_pandoc = ">=0.0.0"
checksum_algorithm = "sha256"
checksum_file = "checksums.sha256"
payload_roots = ["payload"]
required_resources = ["payload/resource.txt"]
"#,
        )
        .expect("manifest");
        fs::write(
            root.join("checksums.sha256"),
            format!(
                "{:x}  payload/resource.txt\n",
                Sha256::digest(checksum_payload)
            ),
        )
        .expect("checksums");
    }

    fn append_archive_file(builder: &mut Builder<GzEncoder<Vec<u8>>>, path: &str, bytes: &[u8]) {
        let mut header = Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, path, Cursor::new(bytes))
            .expect("append archive file");
    }

    fn release_archive(root: &str) -> Vec<u8> {
        let payload = b"verified archive payload\n";
        let manifest = br#"manifest_version = 1
version = "1.0.0"
compatible_omnidoc = ">=1.3.0,<2.0.0"
compatible_pandoc = ">=0.0.0"
checksum_algorithm = "sha256"
checksum_file = "checksums.sha256"
payload_roots = ["payload"]
required_resources = ["payload/resource.txt"]
"#;
        let checksums = format!("{:x}  payload/resource.txt\n", Sha256::digest(payload));
        let encoder = GzEncoder::new(Vec::new(), Compression::default());
        let mut builder = Builder::new(encoder);
        append_archive_file(&mut builder, &format!("{root}/manifest.toml"), manifest);
        append_archive_file(
            &mut builder,
            &format!("{root}/checksums.sha256"),
            checksums.as_bytes(),
        );
        append_archive_file(
            &mut builder,
            &format!("{root}/payload/resource.txt"),
            payload,
        );
        builder
            .into_inner()
            .expect("finish tar archive")
            .finish()
            .expect("finish gzip archive")
    }

    #[test]
    fn rejects_unsafe_manifest_paths() {
        assert!(safe_relative_path("pandoc/css/theme.css").is_some());
        assert!(safe_relative_path("../outside").is_none());
        assert!(safe_relative_path("/absolute").is_none());
    }

    #[test]
    fn evaluates_manifest_version_requirements() {
        assert_eq!(version_matches(">=1.3.0,<2.0.0", "1.3.0"), Some(true));
        assert_eq!(version_matches(">=1.3.0,<2.0.0", "2.0.0"), Some(false));
        assert_eq!(version_matches(">=3.0,<4.0", "pandoc 3.10"), Some(true));
    }

    #[test]
    fn verifies_and_extracts_release_archive() {
        let archive = release_archive("omnidoc-libs-v1.0.0");
        let checksum = format!(
            "{:x}  omnidoc-libs-v1.0.0.tar.gz\n",
            Sha256::digest(&archive)
        );
        verify_release_archive_checksum(
            &archive,
            checksum.as_bytes(),
            "https://example.invalid/omnidoc-libs-v1.0.0.tar.gz",
        )
        .expect("archive checksum");

        let destination = TempDir::new().expect("archive destination");
        extract_release_archive(&archive, destination.path(), "1.0.0")
            .expect("extract release archive");
        let installed = destination.path().join("omnidoc-libs-v1.0.0");
        write_installed_release(
            &installed,
            &InstalledRelease {
                contract_version: 1,
                version: "1.0.0".to_string(),
                revision: "v1.0.0".to_string(),
                archive_url: "https://example.invalid/omnidoc-libs-v1.0.0.tar.gz".to_string(),
                archive_digest: format!("sha256:{:x}", Sha256::digest(&archive)),
            },
        )
        .expect("release metadata");
        let status = inspect_library(&installed, None);
        assert!(status.integrity_verified, "{:?}", status.errors);
        assert_eq!(status.version.as_deref(), Some("1.0.0"));
        assert_eq!(status.source.as_deref(), Some("release"));
        assert_eq!(status.revision.as_deref(), Some("v1.0.0"));
    }

    #[test]
    fn rejects_mismatched_or_unexpected_release_archives() {
        let archive = release_archive("omnidoc-libs-v1.0.0");
        let error = verify_release_archive_checksum(
            &archive,
            b"0000000000000000000000000000000000000000000000000000000000000000  archive.tar.gz\n",
            "https://example.invalid/archive.tar.gz",
        )
        .expect_err("mismatched checksum must fail");
        assert!(error.to_string().contains("checksum mismatch"));

        let unexpected = release_archive("different-root");
        let destination = TempDir::new().expect("archive destination");
        let error = extract_release_archive(&unexpected, destination.path(), "1.0.0")
            .expect_err("unexpected root must fail");
        assert!(error
            .to_string()
            .contains("unexpected release archive path"));
    }

    #[test]
    fn rejects_invalid_manifest_version_requirements() {
        let root = TempDir::new().expect("temporary library");
        fs::write(
            root.path().join("manifest.toml"),
            r#"manifest_version = 1
version = "1.0.0"
compatible_omnidoc = "not a requirement"
compatible_pandoc = ">=3.0,<4.0"
checksum_algorithm = "sha256"
checksum_file = "checksums.sha256"
payload_roots = ["pandoc"]
required_resources = []
"#,
        )
        .expect("manifest");

        let error = read_manifest(root.path()).expect_err("invalid requirement must fail");
        assert!(error.contains("invalid OmniDoc compatibility requirement"));
    }

    #[test]
    fn failed_transactional_update_preserves_installed_library() {
        let root = TempDir::new().expect("temporary root");
        let source = root.path().join("source");
        let installed = root.path().join("installed");
        fs::create_dir_all(&source).expect("source directory");
        let mut init = RepositoryInitOptions::new();
        init.initial_head(git_refs::MAIN_BRANCH);
        let repository = Repository::init_opts(&source, &init).expect("source repository");
        let original = b"verified payload\n";
        write_library_contract(&source, original, original);
        let original_revision = commit_all(&repository, "valid library");

        install_verified_library(
            source.to_str().expect("source URL"),
            &installed,
            None,
            false,
            true,
        )
        .expect("initial installation");
        assert_eq!(
            fs::read(installed.join("payload/resource.txt")).expect("installed payload"),
            original
        );

        write_library_contract(&source, b"tampered payload\n", original);
        commit_all(&repository, "invalid library");
        let error = install_verified_library(
            source.to_str().expect("source URL"),
            &installed,
            None,
            true,
            true,
        )
        .expect_err("invalid update must fail");
        assert!(error.to_string().contains("verification failed"));
        assert_eq!(
            fs::read(installed.join("payload/resource.txt")).expect("preserved payload"),
            original
        );
        assert_eq!(
            Repository::open(&installed)
                .expect("installed repository")
                .head()
                .expect("installed head")
                .target(),
            Some(original_revision)
        );
        assert!(!fs::read_dir(root.path())
            .expect("transaction directory")
            .flatten()
            .any(|entry| entry.file_name().to_string_lossy().contains(".staging.")));

        let updated = b"updated verified payload\n";
        write_library_contract(&source, updated, updated);
        let updated_revision = commit_all(&repository, "valid update");
        install_verified_library(
            source.to_str().expect("source URL"),
            &installed,
            None,
            true,
            true,
        )
        .expect("valid update");
        assert_eq!(
            fs::read(installed.join("payload/resource.txt")).expect("updated payload"),
            updated
        );
        assert_eq!(
            Repository::open(&installed)
                .expect("updated repository")
                .head()
                .expect("updated head")
                .target(),
            Some(updated_revision)
        );
        assert!(!fs::read_dir(root.path())
            .expect("transaction directory")
            .flatten()
            .any(|entry| {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                name.contains(".staging.") || name.contains(".backup.")
            }));
    }
}
