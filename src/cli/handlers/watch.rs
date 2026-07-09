use crate::cli::handlers::build::{build_cli_overrides, build_project_outputs, BuildRunOptions};
use crate::cli::handlers::common::check_omnidoc_project;
use crate::error::{OmniDocError, Result};
use crate::utils::path;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Handle the 'watch' command using the notify backend.
#[allow(clippy::too_many_arguments)]
pub fn handle_watch(
    path: Option<String>,
    to: Option<String>,
    all: bool,
    outputs: Vec<String>,
    pdf_engine: Option<String>,
    latex_backend: String,
    max_latex_passes: Option<usize>,
    debounce_ms: u64,
    once: bool,
    force: bool,
    report: bool,
    strict: bool,
    verbose: bool,
) -> Result<()> {
    let project_path = path::determine_project_path(path)?.canonicalize()?;
    check_omnidoc_project(&project_path)?;

    let cli_overrides = build_cli_overrides(
        to,
        outputs,
        pdf_engine,
        latex_backend,
        max_latex_passes,
        verbose,
    );
    let run_options = BuildRunOptions {
        force,
        report,
        write_lock: false,
        strict,
    };

    println!("Watching {} with notify", project_path.display());
    run_watch_build(
        &project_path,
        cli_overrides.clone(),
        all,
        run_options.clone(),
        verbose,
    );
    if once {
        return Ok(());
    }

    let (tx, rx) = mpsc::channel();
    let mut watcher = RecommendedWatcher::new(
        move |event| {
            let _ = tx.send(event);
        },
        Config::default(),
    )
    .map_err(|err| OmniDocError::Other(format!("Failed to create watcher: {}", err)))?;
    watcher
        .watch(&project_path, RecursiveMode::Recursive)
        .map_err(|err| OmniDocError::Other(format!("Failed to watch project: {}", err)))?;

    let debounce = Duration::from_millis(debounce_ms.max(50));
    let mut pending = Vec::new();
    let mut last_event: Option<Instant> = None;

    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(Ok(event)) => {
                if should_rebuild_for_event(&event) {
                    pending.extend(event.paths);
                    last_event = Some(Instant::now());
                }
            }
            Ok(Err(err)) => eprintln!("watch error: {}", err),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(OmniDocError::Other(
                    "watch event channel disconnected".to_string(),
                ));
            }
        }

        if last_event
            .map(|instant| instant.elapsed() >= debounce)
            .unwrap_or(false)
        {
            let changed = compact_changed_paths(&project_path, &pending);
            println!("Change detected: {}", changed.join(", "));
            run_watch_build(
                &project_path,
                cli_overrides.clone(),
                all,
                run_options.clone(),
                verbose,
            );
            pending.clear();
            last_event = None;
        }
    }
}

fn run_watch_build(
    project_path: &std::path::Path,
    cli_overrides: crate::config::CliOverrides,
    all: bool,
    run_options: BuildRunOptions,
    verbose: bool,
) {
    match build_project_outputs(project_path, cli_overrides, all, run_options, verbose) {
        Ok(()) => println!("Build completed."),
        Err(err) => eprintln!("Build failed:\n{}", err),
    }
}

fn should_rebuild_for_event(event: &Event) -> bool {
    event.paths.iter().any(|path| {
        should_watch_path(path)
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| !name.starts_with('.'))
                .unwrap_or(true)
    })
}

fn should_watch_path(path: &std::path::Path) -> bool {
    if path.components().any(|component| {
        let value = component.as_os_str().to_string_lossy();
        matches!(
            value.as_ref(),
            ".git" | "build" | "target" | ".target" | ".cache" | ".omnidoc-cache" | "node_modules"
        )
    }) {
        return false;
    }

    let Some(ext) = path.extension().and_then(|value| value.to_str()) else {
        return path.file_name().and_then(|name| name.to_str()) == Some(".omnidoc.toml");
    };
    matches!(
        ext.to_ascii_lowercase().as_str(),
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

fn compact_changed_paths(project_path: &std::path::Path, paths: &[PathBuf]) -> Vec<String> {
    let mut values = paths
        .iter()
        .filter(|path| should_watch_path(path))
        .map(|path| path.strip_prefix(project_path).unwrap_or(path))
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

#[cfg(test)]
mod tests {
    use super::should_watch_path;
    use std::path::Path;

    #[test]
    fn ignores_build_directory() {
        assert!(!should_watch_path(Path::new("build/output.pdf")));
        assert!(should_watch_path(Path::new("main.md")));
    }
}
