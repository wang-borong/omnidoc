use crate::cli::handlers::build::{
    build_cli_overrides, build_project_outputs, resolve_outputs, BuildRunOptions,
};
use crate::cli::handlers::common::{check_omnidoc_project, create_config_manager};
use crate::config::MergedConfig;
use crate::doc::artifacts::{artifact_for_format, output_directory};
use crate::error::{OmniDocError, Result};
use crate::extensions::{
    acquire_extension_store_read_locks, extension_store_roots, plugin_trust_path,
};
use crate::project_tools;
use crate::terminal;
use crate::utils::directories::data_local_dir;
use crate::utils::path;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::BTreeSet;
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
    let mut watch_context = WatchContext::load(&project_path, &cli_overrides, all)?;

    println!("Watching {} with notify", project_path.display());
    if once {
        return run_watch_build(&project_path, cli_overrides, all, run_options, verbose);
    }

    let (tx, rx) = mpsc::channel();
    let mut watcher = RecommendedWatcher::new(
        move |event| {
            let _ = tx.send(event);
        },
        Config::default(),
    )
    .map_err(|err| OmniDocError::Other(format!("Failed to create watcher: {}", err)))?;
    let mut active_watches = BTreeSet::new();
    apply_watch_registrations(
        &mut watcher,
        &mut active_watches,
        &watch_context.registrations,
        &project_path,
    )?;

    let initial_build = run_watch_build(
        &project_path,
        cli_overrides.clone(),
        all,
        run_options.clone(),
        verbose,
    );
    if let Err(error) = initial_build {
        terminal::print_error(&error);
    }
    refresh_watch_context(
        &project_path,
        &cli_overrides,
        all,
        &mut watcher,
        &mut active_watches,
        &mut watch_context,
        &BTreeSet::new(),
    );

    let debounce = Duration::from_millis(debounce_ms.max(50));
    let mut pending = Vec::new();
    let mut last_event: Option<Instant> = None;

    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(Ok(event)) => {
                if should_rebuild_for_event(&event, &watch_context.filter) {
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
            let changed = compact_changed_paths(&watch_context.filter, &pending);
            let recursive_roots_to_rearm =
                recursive_roots_requiring_rearm(&watch_context.filter, &pending);
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
            refresh_watch_context(
                &project_path,
                &cli_overrides,
                all,
                &mut watcher,
                &mut active_watches,
                &mut watch_context,
                &recursive_roots_to_rearm,
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
) -> Result<()> {
    build_project_outputs(project_path, cli_overrides, all, run_options, verbose)?;
    println!("Build completed.");
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct WatchRegistration {
    path: PathBuf,
    recursive: bool,
}

struct WatchContext {
    filter: WatchFilter,
    registrations: BTreeSet<WatchRegistration>,
}

impl WatchContext {
    fn load(
        project_path: &Path,
        cli_overrides: &crate::config::CliOverrides,
        all: bool,
    ) -> Result<Self> {
        let manager = create_config_manager(Some(project_path), cli_overrides.clone())?;
        let config = manager.get_merged().clone();
        let outputs = resolve_outputs(&config, cli_overrides, all);
        let extensions_active = config.theme_name.is_some() || !config.plugins_enabled.is_empty();
        let _extension_locks = extensions_active
            .then(|| {
                acquire_extension_store_read_locks(
                    Some(project_path),
                    &config,
                    "refresh watched extension inputs",
                )
            })
            .transpose()?;

        let mut tracked_files = BTreeSet::new();
        let mut resource_paths = BTreeSet::new();
        for output in &outputs {
            let mut output_config = config.clone();
            output_config.to = Some(output.clone());
            let graph = project_tools::dependency_graph(project_path, &output_config);
            tracked_files.extend(
                graph
                    .files
                    .into_iter()
                    .map(|file| absolute_watch_path(project_path, Path::new(&file))),
            );
            resource_paths.extend(
                graph
                    .resources
                    .into_iter()
                    .map(|resource| absolute_watch_path(project_path, Path::new(&resource.path))),
            );
        }

        let mut extension_roots = Vec::new();
        if extensions_active {
            extension_roots.extend(extension_store_roots(Some(project_path), &config)?);
        }
        let mut external_roots = Vec::new();
        let library_root = configured_library_root(&config);
        if library_root.is_dir() {
            external_roots.push(library_root.clone());
        }
        let mut external_files = vec![manager.global().path().to_path_buf(), library_root];
        if !config.plugins_enabled.is_empty() {
            external_files.push(plugin_trust_path()?);
        }

        let normalized_project = normalize_path(project_path.to_path_buf());
        for resource in resource_paths {
            if resource.starts_with(&normalized_project) {
                tracked_files.insert(resource);
            } else if resource.is_dir() {
                external_roots.push(resource);
            } else {
                external_files.push(resource);
            }
        }

        let filter = WatchFilter::with_dependencies(
            project_path,
            &config,
            &outputs,
            tracked_files,
            extension_roots,
            external_roots,
            external_files,
        )?;
        let registrations = watch_registrations(&filter);
        Ok(Self {
            filter,
            registrations,
        })
    }
}

fn configured_library_root(config: &MergedConfig) -> PathBuf {
    let current = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    config
        .lib_path
        .as_ref()
        .map(PathBuf::from)
        .or_else(|| data_local_dir().map(|path| path.join("omnidoc")))
        .map(|path| absolute_watch_path(&current, &path))
        .unwrap_or_else(|| absolute_watch_path(&current, Path::new(".local/share/omnidoc")))
}

fn absolute_watch_path(base: &Path, path: &Path) -> PathBuf {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    };
    path.canonicalize().unwrap_or_else(|_| normalize_path(path))
}

fn watch_registrations(filter: &WatchFilter) -> BTreeSet<WatchRegistration> {
    let mut registrations = BTreeSet::from([WatchRegistration {
        path: filter.project_path.clone(),
        recursive: true,
    }]);
    for root in filter
        .extension_roots
        .iter()
        .chain(filter.external_roots.iter())
    {
        if root.is_dir() {
            registrations.insert(WatchRegistration {
                path: root.clone(),
                recursive: true,
            });
        }
        if let Some(parent) = nearest_existing_directory(root.parent()) {
            registrations.insert(WatchRegistration {
                path: parent,
                recursive: false,
            });
        }
    }
    for file in &filter.external_files {
        if let Some(parent) = nearest_existing_directory(file.parent()) {
            registrations.insert(WatchRegistration {
                path: parent,
                recursive: false,
            });
        }
    }

    let recursive_roots = registrations
        .iter()
        .filter(|registration| registration.recursive)
        .map(|registration| registration.path.clone())
        .collect::<Vec<_>>();
    registrations.retain(|registration| {
        (registration.recursive || !recursive_roots.contains(&registration.path))
            && !recursive_roots
                .iter()
                .any(|root| root != &registration.path && registration.path.starts_with(root))
    });
    registrations
}

fn nearest_existing_directory(mut path: Option<&Path>) -> Option<PathBuf> {
    while let Some(candidate) = path {
        if candidate.is_dir() {
            return Some(normalize_path(candidate.to_path_buf()));
        }
        path = candidate.parent();
    }
    None
}

fn apply_watch_registrations(
    watcher: &mut RecommendedWatcher,
    active: &mut BTreeSet<WatchRegistration>,
    desired: &BTreeSet<WatchRegistration>,
    project_path: &Path,
) -> Result<()> {
    let removed = active.difference(desired).cloned().collect::<Vec<_>>();
    for registration in removed {
        if let Err(error) = watcher.unwatch(&registration.path) {
            terminal::warning(format!(
                "Could not remove stale watch for {}\n{}",
                registration.path.display(),
                error
            ));
        }
        active.remove(&registration);
    }

    let added = desired.difference(active).cloned().collect::<Vec<_>>();
    for registration in added {
        let mode = if registration.recursive {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };
        if let Err(error) = watcher.watch(&registration.path, mode) {
            if registration.path == project_path {
                return Err(OmniDocError::Other(format!(
                    "Failed to watch project: {error}"
                )));
            }
            terminal::warning(format!(
                "Could not watch external build input {}\n{}",
                registration.path.display(),
                error
            ));
            continue;
        }
        active.insert(registration);
    }
    Ok(())
}

fn refresh_watch_context(
    project_path: &Path,
    cli_overrides: &crate::config::CliOverrides,
    all: bool,
    watcher: &mut RecommendedWatcher,
    active: &mut BTreeSet<WatchRegistration>,
    context: &mut WatchContext,
    recursive_roots_to_rearm: &BTreeSet<PathBuf>,
) {
    let refreshed = WatchContext::load(project_path, cli_overrides, all).and_then(|next| {
        let rearm = active
            .iter()
            .filter(|registration| {
                registration.recursive
                    && registration.path != project_path
                    && recursive_roots_to_rearm.contains(&registration.path)
            })
            .cloned()
            .collect::<Vec<_>>();
        for registration in rearm {
            let _ = watcher.unwatch(&registration.path);
            active.remove(&registration);
        }
        apply_watch_registrations(watcher, active, &next.registrations, project_path)?;
        Ok(next)
    });
    match refreshed {
        Ok(next) => *context = next,
        Err(error) => terminal::warning(format!(
            "Could not refresh watched build inputs; retaining the previous watch set\n{error}"
        )),
    }
}

fn recursive_roots_requiring_rearm(
    filter: &WatchFilter,
    event_paths: &[PathBuf],
) -> BTreeSet<PathBuf> {
    let event_paths = event_paths
        .iter()
        .map(|path| absolute_watch_path(&filter.project_path, path))
        .collect::<Vec<_>>();

    filter
        .extension_roots
        .iter()
        .chain(filter.external_roots.iter())
        .filter(|root| {
            event_paths
                .iter()
                .any(|event_path| root.starts_with(event_path))
        })
        .cloned()
        .collect()
}

fn should_rebuild_for_event(event: &Event, filter: &WatchFilter) -> bool {
    if event.kind.is_access()
        && !matches!(
            event.kind,
            notify::EventKind::Access(notify::event::AccessKind::Close(
                notify::event::AccessMode::Write
            ))
        )
    {
        return false;
    }
    event
        .paths
        .iter()
        .any(|path| filter.should_watch_path(path))
}

struct WatchFilter {
    project_path: PathBuf,
    ignored_roots: Vec<PathBuf>,
    ignored_files: Vec<PathBuf>,
    tracked_files: BTreeSet<PathBuf>,
    extension_roots: Vec<PathBuf>,
    external_roots: Vec<PathBuf>,
    external_files: BTreeSet<PathBuf>,
}

impl WatchFilter {
    #[cfg(test)]
    fn new(project_path: &Path, config: &MergedConfig, outputs: &[String]) -> Result<Self> {
        Self::with_dependencies(
            project_path,
            config,
            outputs,
            BTreeSet::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn with_dependencies(
        project_path: &Path,
        config: &MergedConfig,
        outputs: &[String],
        tracked_files: BTreeSet<PathBuf>,
        extension_roots: Vec<PathBuf>,
        external_roots: Vec<PathBuf>,
        external_files: Vec<PathBuf>,
    ) -> Result<Self> {
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

        let tracked_files = tracked_files
            .into_iter()
            .map(normalize_path)
            .collect::<BTreeSet<_>>();
        let mut extension_roots = extension_roots
            .into_iter()
            .map(|path| absolute_watch_path(&project_path, &path))
            .collect::<Vec<_>>();
        extension_roots.sort();
        extension_roots.dedup();
        let mut external_roots = external_roots
            .into_iter()
            .map(|path| absolute_watch_path(&project_path, &path))
            .collect::<Vec<_>>();
        external_roots.sort();
        external_roots.dedup();
        let external_files = external_files
            .into_iter()
            .map(|path| absolute_watch_path(&project_path, &path))
            .collect::<BTreeSet<_>>();

        Ok(Self {
            project_path,
            ignored_roots,
            ignored_files,
            tracked_files,
            extension_roots,
            external_roots,
            external_files,
        })
    }

    fn should_watch_path(&self, path: &Path) -> bool {
        let absolute = absolute_watch_path(&self.project_path, path);
        let extension_root = self
            .extension_roots
            .iter()
            .filter(|root| absolute.starts_with(root))
            .max_by_key(|root| root.components().count());
        if let Some(root) = extension_root {
            return !is_extension_store_internal_path(root, &absolute);
        }
        if self
            .extension_roots
            .iter()
            .any(|root| root.starts_with(&absolute))
        {
            return true;
        }
        if self
            .external_roots
            .iter()
            .any(|root| absolute.starts_with(root) || root.starts_with(&absolute))
            || self
                .external_files
                .iter()
                .any(|file| file == &absolute || file.starts_with(&absolute))
        {
            return true;
        }
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
        if self.tracked_files.contains(&absolute) {
            return true;
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
                | "eps"
                | "gif"
                | "webp"
                | "avif"
                | "csv"
                | "tsv"
                | "lua"
                | "css"
                | "scss"
                | "sass"
                | "less"
                | "toml"
                | "html"
                | "htm"
                | "xhtml"
                | "xml"
                | "csl"
                | "docx"
                | "pptx"
                | "odt"
                | "rtf"
                | "ttf"
                | "otf"
                | "woff"
                | "woff2"
        )
    }
}

fn is_extension_store_internal_path(root: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };
    let mut components = relative.components();
    let Some(first) = components.next() else {
        return false;
    };
    if first.as_os_str() == ".transactions" {
        return true;
    }
    first.as_os_str() == ".omnidoc-store.lock" && components.next().is_none()
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
    use super::{
        recursive_roots_requiring_rearm, should_rebuild_for_event, watch_registrations, WatchFilter,
    };
    use crate::config::MergedConfig;
    use notify::{Event, EventKind};
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};

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

    #[test]
    fn project_configuration_events_trigger_rebuilds() {
        let project = tempfile::tempdir().expect("project");
        let filter = WatchFilter::new(project.path(), &MergedConfig::default(), &["html".into()])
            .expect("watch filter");
        let event = Event::new(EventKind::Any).add_path(project.path().join(".omnidoc.toml"));

        assert!(should_rebuild_for_event(&event, &filter));
    }

    #[test]
    fn read_access_events_do_not_trigger_rebuilds() {
        let project = tempfile::tempdir().expect("project");
        let filter = WatchFilter::new(project.path(), &MergedConfig::default(), &["html".into()])
            .expect("watch filter");
        let event = Event::new(EventKind::Access(notify::event::AccessKind::Open(
            notify::event::AccessMode::Read,
        )))
        .add_path(project.path().join("main.md"));

        assert!(!should_rebuild_for_event(&event, &filter));
    }

    #[test]
    fn close_write_access_events_trigger_rebuilds() {
        let project = tempfile::tempdir().expect("project");
        let filter = WatchFilter::new(project.path(), &MergedConfig::default(), &["html".into()])
            .expect("watch filter");
        let event = Event::new(EventKind::Access(notify::event::AccessKind::Close(
            notify::event::AccessMode::Write,
        )))
        .add_path(project.path().join("main.md"));

        assert!(should_rebuild_for_event(&event, &filter));
    }

    #[test]
    fn extension_payloads_are_watched_but_transaction_noise_is_ignored() {
        let workspace = tempfile::tempdir().expect("workspace");
        let project = workspace.path().join("project");
        let store = workspace.path().join("extensions");
        fs::create_dir_all(&project).expect("project");
        fs::create_dir_all(&store).expect("extension store");
        let filter = WatchFilter::with_dependencies(
            &project,
            &MergedConfig::default(),
            &["html".into()],
            BTreeSet::new(),
            vec![store.clone()],
            Vec::new(),
            Vec::new(),
        )
        .expect("extension watch filter");

        assert!(
            filter.should_watch_path(&store.join("plugins/acme/check/1.0.0/omnidoc-package.toml"))
        );
        assert!(filter.should_watch_path(&store.join("plugins/acme/check/1.0.0/filters/main.lua")));
        assert!(!filter.should_watch_path(&store.join(".omnidoc-store.lock")));
        assert!(!filter
            .should_watch_path(&store.join(".transactions/installing/payload/filters/main.lua")));
        assert!(filter
            .should_watch_path(&store.join("plugins/acme/check/1.0.0/.transactions/main.lua")));
        assert!(
            filter.should_watch_path(&store.join("plugins/acme/check/1.0.0/.omnidoc-store.lock"))
        );
    }

    #[test]
    fn explicitly_tracked_inputs_are_watched_regardless_of_extension() {
        let project = tempfile::tempdir().expect("project");
        let tracked = project.path().join("assets/custom.input");
        let filter = WatchFilter::with_dependencies(
            project.path(),
            &MergedConfig::default(),
            &["html".into()],
            BTreeSet::from([tracked.clone()]),
            Vec::new(),
            Vec::new(),
            Vec::<PathBuf>::new(),
        )
        .expect("tracked input filter");

        assert!(filter.should_watch_path(&tracked));
    }

    #[test]
    fn explicitly_tracked_inputs_override_generic_directory_ignores() {
        let project = tempfile::tempdir().expect("project");
        let tracked = project.path().join("vendor/target/custom.input");
        let filter = WatchFilter::with_dependencies(
            project.path(),
            &MergedConfig::default(),
            &["html".into()],
            BTreeSet::from([tracked.clone()]),
            Vec::new(),
            Vec::new(),
            Vec::<PathBuf>::new(),
        )
        .expect("tracked input filter");

        assert!(filter.should_watch_path(&tracked));
    }

    #[test]
    fn missing_external_file_is_watched_from_its_nearest_existing_ancestor() {
        let workspace = tempfile::tempdir().expect("workspace");
        let project = workspace.path().join("project");
        let existing = workspace.path().join("config");
        let missing_file = existing.join("nested/trust.json");
        fs::create_dir_all(&project).expect("project");
        fs::create_dir_all(&existing).expect("existing ancestor");
        let filter = WatchFilter::with_dependencies(
            &project,
            &MergedConfig::default(),
            &["html".into()],
            BTreeSet::new(),
            Vec::new(),
            Vec::new(),
            vec![missing_file.clone()],
        )
        .expect("external file filter");
        let registrations = watch_registrations(&filter);

        assert!(registrations
            .iter()
            .any(|registration| registration.path == existing && !registration.recursive));
        assert!(filter.should_watch_path(&existing.join("nested")));
        assert!(filter.should_watch_path(&missing_file));
    }

    #[test]
    fn nested_extension_changes_do_not_rearm_recursive_store_watches() {
        let workspace = tempfile::tempdir().expect("workspace");
        let project = workspace.path().join("project");
        let store = workspace.path().join("extensions");
        fs::create_dir_all(&project).expect("project");
        fs::create_dir_all(&store).expect("extension store");
        let filter = WatchFilter::with_dependencies(
            &project,
            &MergedConfig::default(),
            &["html".into()],
            BTreeSet::new(),
            vec![store.clone()],
            Vec::new(),
            Vec::new(),
        )
        .expect("extension watch filter");

        let rearm = recursive_roots_requiring_rearm(
            &filter,
            &[store.join("plugins/acme/check/1.0.0/filters/main.lua")],
        );

        assert!(rearm.is_empty());
    }

    #[test]
    fn recursive_store_watches_rearm_for_root_or_ancestor_replacement() {
        let workspace = tempfile::tempdir().expect("workspace");
        let project = workspace.path().join("project");
        let store = workspace.path().join("managed/extensions");
        fs::create_dir_all(&project).expect("project");
        fs::create_dir_all(&store).expect("extension store");
        let filter = WatchFilter::with_dependencies(
            &project,
            &MergedConfig::default(),
            &["html".into()],
            BTreeSet::new(),
            vec![store.clone()],
            Vec::new(),
            Vec::new(),
        )
        .expect("extension watch filter");

        assert_eq!(
            recursive_roots_requiring_rearm(&filter, std::slice::from_ref(&store)),
            BTreeSet::from([store.clone()])
        );
        assert_eq!(
            recursive_roots_requiring_rearm(
                &filter,
                &[store.parent().expect("store parent").to_path_buf()],
            ),
            BTreeSet::from([store])
        );
    }

    #[test]
    fn missing_extension_store_creation_is_watched_through_its_ancestors() {
        let workspace = tempfile::tempdir().expect("workspace");
        let project = workspace.path().join("project");
        let store = workspace.path().join("managed/extensions");
        fs::create_dir_all(&project).expect("project");
        let filter = WatchFilter::with_dependencies(
            &project,
            &MergedConfig::default(),
            &["html".into()],
            BTreeSet::new(),
            vec![store.clone()],
            Vec::new(),
            Vec::new(),
        )
        .expect("extension watch filter");
        let registrations = watch_registrations(&filter);

        assert!(registrations.iter().any(|registration| {
            registration.path == workspace.path() && !registration.recursive
        }));
        assert!(filter.should_watch_path(&workspace.path().join("managed")));
        assert!(filter.should_watch_path(&store));
    }
}
