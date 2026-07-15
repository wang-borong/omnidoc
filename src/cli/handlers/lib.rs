use crate::config::global::GlobalConfig;
use crate::constants::config as config_consts;
use crate::constants::git_refs;
use crate::doc::templates::get_latexmkrc_template;
use crate::error::{OmniDocError, Result};
use crate::git::{git_checkout_revision, git_clone};
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
use std::time::{SystemTime, UNIX_EPOCH};
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
        install_verified_library(
            &lib_url,
            &library_path,
            requested_revision.as_deref(),
            false,
            json,
        )?;
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
        let lib_url = installed_library_url(&library_path)
            .unwrap_or_else(|| configured_library_url(&global_config));
        install_verified_library(
            &lib_url,
            &library_path,
            requested_revision.as_deref(),
            true,
            json,
        )?;
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

    let backup = sibling_transaction_path(library_path, "backup");
    if let Err(error) = fs::rename(library_path, &backup) {
        cleanup_transaction_path(&staging);
        return Err(error);
    }
    if let Err(error) = fs::rename(&staging, library_path) {
        let restore = fs::rename(&backup, library_path);
        cleanup_transaction_path(&staging);
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
    use super::{
        git_refs, install_verified_library, read_manifest, safe_relative_path, version_matches,
    };
    use git2::{Repository, RepositoryInitOptions, Signature};
    use sha2::{Digest, Sha256};
    use std::fs;
    use std::path::Path;
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
