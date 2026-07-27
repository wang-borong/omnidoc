use crate::cli::handlers::build::{
    build_cli_overrides, build_project_outputs, resolve_outputs, BuildRunOptions,
};
use crate::cli::handlers::common::{check_omnidoc_project, create_config_manager};
use crate::config::MergedConfig;
use crate::doc::artifacts::{artifact_for_format, output_directory};
use crate::error::{OmniDocError, Result};
use crate::terminal;
use crate::utils::path;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
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
    latex_backend: Option<String>,
    max_latex_passes: Option<usize>,
    debounce_ms: u64,
    once: bool,
    force: bool,
    report: bool,
    strict: bool,
    verbose: bool,
) -> Result<()> {
    let project_path = path::determine_project_root(path)?;
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
    let watch_config = create_config_manager(Some(&project_path), cli_overrides.clone())?;
    let watch_outputs = resolve_outputs(watch_config.get_merged(), &cli_overrides, all);
    let watch_filter = WatchFilter::new(&project_path, watch_config.get_merged(), &watch_outputs)?;

    println!("Watching {} with notify", project_path.display());
    let initial_build = run_watch_build(
        &project_path,
        cli_overrides.clone(),
        all,
        run_options.clone(),
        verbose,
    );
    if once {
        return initial_build;
    }
    if let Err(error) = initial_build {
        terminal::print_error(&error);
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
                if should_rebuild_for_event(&event, &watch_filter) {
                    pending.extend(event.paths);
                    last_event = Some(Instant::now());
                }
            }
            Ok(Err(err)) => terminal::warning(format!("File watcher reported an error\n{err}")),
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
            let changed = compact_changed_paths(&watch_filter, &pending);
            println!("Change detected: {}", changed.join(", "));
            if let Err(error) = run_watch_build(
                &project_path,
                cli_overrides.clone(),
                all,
                run_options.clone(),
                verbose,
            ) {
                terminal::print_error(&error);
            }
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
) -> Result<()> {
    build_project_outputs(project_path, cli_overrides, all, run_options, verbose)?;
    println!("Build completed.");
    Ok(())
}

fn should_rebuild_for_event(event: &Event, filter: &WatchFilter) -> bool {
    event.paths.iter().any(|path| {
        filter.should_watch_path(path)
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| !name.starts_with('.'))
                .unwrap_or(true)
    })
}

struct WatchFilter {
    project_path: PathBuf,
    ignored_roots: Vec<PathBuf>,
    ignored_files: Vec<PathBuf>,
}

impl WatchFilter {
    fn new(project_path: &Path, config: &MergedConfig, outputs: &[String]) -> Result<Self> {
        let project_path = normalize_path(project_path.to_path_buf());
        let outdir = normalize_path(output_directory(&project_path, config));
        let figure_output_name = config
            .figure_output
            .as_deref()
            .filter(|path| !path.trim().is_empty())
            .or_else(|| {
                (!config.paths.figures_dir.trim().is_empty())
                    .then_some(config.paths.figures_dir.as_str())
            })
            .unwrap_or("figures");
        let figure_output = normalize_path(project_path.join(figure_output_name));
        let mut ignored_roots = vec![normalize_path(project_path.join("dist")), figure_output];
        if outdir != project_path {
            ignored_roots.push(outdir.clone());
        }
        ignored_roots.retain(|path| path != &project_path);
        ignored_roots.sort();
        ignored_roots.dedup();

        let mut ignored_files = outputs
            .iter()
            .map(|output| artifact_for_format(&project_path, config, output))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .map(|artifact| normalize_path(PathBuf::from(artifact.path)))
            .collect::<Vec<_>>();
        ignored_files.push(normalize_path(outdir.join("omnidoc-report.json")));
        ignored_files.sort();
        ignored_files.dedup();

        Ok(Self {
            project_path,
            ignored_roots,
            ignored_files,
        })
    }

    fn should_watch_path(&self, path: &Path) -> bool {
        let absolute = normalize_path(if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.project_path.join(path)
        });
        if self.ignored_files.contains(&absolute) {
            return false;
        }
        if self
            .ignored_roots
            .iter()
            .any(|ignored| absolute.starts_with(ignored))
        {
            return false;
        }
        if path.components().any(|component| {
            let value = component.as_os_str().to_string_lossy();
            matches!(
                value.as_ref(),
                ".git"
                    | "build"
                    | "dist"
                    | "target"
                    | ".target"
                    | ".cache"
                    | ".omnidoc-cache"
                    | "node_modules"
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
}

fn normalize_path(path: PathBuf) -> PathBuf {
    path.components().collect()
}

fn compact_changed_paths(filter: &WatchFilter, paths: &[PathBuf]) -> Vec<String> {
    let mut values = paths
        .iter()
        .filter(|path| filter.should_watch_path(path))
        .map(|path| path.strip_prefix(&filter.project_path).unwrap_or(path))
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

#[cfg(test)]
mod tests {
    use super::WatchFilter;
    use crate::config::MergedConfig;
    use std::path::Path;

    #[test]
    fn ignores_default_and_configured_generated_directories() {
        let root = Path::new("/tmp/project");
        let filter = WatchFilter::new(
            root,
            &MergedConfig {
                outdir: Some("site-output".to_string()),
                ..Default::default()
            },
            &["pdf".to_string()],
        )
        .expect("watch filter");

        assert!(!filter.should_watch_path(Path::new("build/output.pdf")));
        assert!(!filter.should_watch_path(Path::new("site-output/book.html")));
        assert!(!filter.should_watch_path(Path::new("dist/release/book.pdf")));
        assert!(!filter.should_watch_path(Path::new("figures/generated.svg")));
        assert!(filter.should_watch_path(Path::new("main.md")));
        assert!(filter.should_watch_path(Path::new("notes/site-output.md")));
    }

    #[test]
    fn root_output_directory_ignores_only_generated_artifacts() {
        let root = Path::new("/tmp/project");
        let filter = WatchFilter::new(
            root,
            &MergedConfig {
                outdir: Some(".".to_string()),
                target: Some("guide".to_string()),
                ..Default::default()
            },
            &["pdf".to_string(), "html".to_string()],
        )
        .expect("root output watch filter");

        assert!(!filter.should_watch_path(Path::new("guide.pdf")));
        assert!(!filter.should_watch_path(Path::new("guide.html")));
        assert!(!filter.should_watch_path(Path::new("omnidoc-report.json")));
        assert!(filter.should_watch_path(Path::new("reference.pdf")));
        assert!(filter.should_watch_path(Path::new("main.md")));
    }
}
