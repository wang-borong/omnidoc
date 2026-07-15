use crate::config::global::GlobalConfig;
use crate::constants::config as config_consts;
use crate::constants::git_refs;
use crate::doc::templates::get_latexmkrc_template;
use crate::error::{OmniDocError, Result};
use crate::git::{git_checkout_revision, git_clone, git_fetch, git_pull};
use crate::utils::fs;
use console::style;
use dirs::{config_local_dir, data_local_dir};
use git2::{Repository, StatusOptions};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

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

#[derive(Debug, Serialize)]
struct LibraryStatus {
    path: String,
    exists: bool,
    manifest_valid: bool,
    integrity_verified: bool,
    version: Option<String>,
    revision: Option<String>,
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
) -> Result<()> {
    let global_config = GlobalConfig::load()?;
    let library_path = configured_library_path(&global_config)?;
    let requested_revision = revision.or_else(|| configured_library_revision(&global_config));
    let requested_install = install || (!update && !status && !verify);

    if requested_install {
        let lib_url = configured_library_url(&global_config);
        git_clone(&lib_url, &library_path, true).map_err(OmniDocError::Git)?;
        if let Some(revision) = requested_revision.as_deref() {
            git_checkout_revision(&library_path, revision).map_err(OmniDocError::Git)?;
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
        ensure_clean_repository(&library_path)?;
        if let Some(revision) = requested_revision.as_deref() {
            git_fetch(&library_path, git_refs::ORIGIN, git_refs::MAIN_BRANCH)
                .map_err(OmniDocError::Git)?;
            git_checkout_revision(&library_path, revision).map_err(OmniDocError::Git)?;
        } else {
            git_pull(&library_path, git_refs::ORIGIN, git_refs::MAIN_BRANCH)
                .map_err(OmniDocError::Git)?;
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
        manifest_valid: false,
        integrity_verified: false,
        version: None,
        revision: None,
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
    println!(
        "version: {}",
        status.version.as_deref().unwrap_or("unknown")
    );
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
    use super::{read_manifest, safe_relative_path, version_matches};
    use std::fs;
    use tempfile::TempDir;

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
}
