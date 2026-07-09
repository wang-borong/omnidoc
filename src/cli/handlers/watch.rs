use crate::cli::handlers::build::{build_cli_overrides, build_project};
use crate::cli::handlers::common::check_omnidoc_project;
use crate::error::Result;
use crate::utils::path;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, UNIX_EPOCH};
use walkdir::WalkDir;

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileFingerprint {
    len: u64,
    modified_nanos: u128,
}

/// Handle the 'watch' command.
pub fn handle_watch(
    path: Option<String>,
    to: Option<String>,
    pdf_engine: Option<String>,
    latex_backend: String,
    max_latex_passes: Option<usize>,
    interval_ms: u64,
    once: bool,
    verbose: bool,
) -> Result<()> {
    let project_path = path::determine_project_path(path)?;
    let project_path = project_path.canonicalize()?;
    check_omnidoc_project(&project_path)?;

    let cli_overrides =
        build_cli_overrides(to, pdf_engine, latex_backend, max_latex_passes, verbose);
    let interval = Duration::from_millis(interval_ms.max(250));

    println!(
        "Watching {} ({} ms)",
        project_path.display(),
        interval.as_millis()
    );
    run_watch_build(&project_path, cli_overrides.clone(), verbose);

    let mut previous = snapshot_project(&project_path)?;
    if once {
        return Ok(());
    }

    loop {
        thread::sleep(interval);
        let current = snapshot_project(&project_path)?;
        let changed = changed_paths(&previous, &current);
        if changed.is_empty() {
            continue;
        }

        println!("Change detected: {}", changed.join(", "));
        run_watch_build(&project_path, cli_overrides.clone(), verbose);
        previous = snapshot_project(&project_path)?;
    }
}

fn run_watch_build(project_path: &Path, cli_overrides: crate::config::CliOverrides, verbose: bool) {
    match build_project(project_path, cli_overrides, verbose) {
        Ok(()) => println!("Build completed."),
        Err(err) => eprintln!("Build failed:\n{}", err),
    }
}

fn snapshot_project(project_path: &Path) -> Result<BTreeMap<PathBuf, FileFingerprint>> {
    let mut snapshot = BTreeMap::new();

    for entry in WalkDir::new(project_path)
        .into_iter()
        .filter_entry(|entry| should_descend(entry.path(), project_path))
    {
        let entry = entry?;
        let path = entry.path();
        if !entry.file_type().is_file() || !is_watched_file(path) {
            continue;
        }

        let metadata = entry.metadata()?;
        let modified = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let relative_path = path
            .strip_prefix(project_path)
            .unwrap_or(path)
            .to_path_buf();

        snapshot.insert(
            relative_path,
            FileFingerprint {
                len: metadata.len(),
                modified_nanos: modified,
            },
        );
    }

    Ok(snapshot)
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
        ".git" | "build" | "target" | ".target" | ".cache" | "node_modules"
    )
}

fn is_watched_file(path: &Path) -> bool {
    let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
        return false;
    };

    matches!(
        extension.to_ascii_lowercase().as_str(),
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

fn changed_paths(
    previous: &BTreeMap<PathBuf, FileFingerprint>,
    current: &BTreeMap<PathBuf, FileFingerprint>,
) -> Vec<String> {
    let mut paths = BTreeSet::new();
    paths.extend(previous.keys().cloned());
    paths.extend(current.keys().cloned());

    paths
        .into_iter()
        .filter(|path| previous.get(path) != current.get(path))
        .map(|path| path.display().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{changed_paths, FileFingerprint};
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    #[test]
    fn detects_changed_paths() {
        let mut previous = BTreeMap::new();
        let mut current = BTreeMap::new();
        previous.insert(
            PathBuf::from("main.md"),
            FileFingerprint {
                len: 1,
                modified_nanos: 1,
            },
        );
        current.insert(
            PathBuf::from("main.md"),
            FileFingerprint {
                len: 2,
                modified_nanos: 2,
            },
        );

        assert_eq!(changed_paths(&previous, &current), vec!["main.md"]);
    }
}
