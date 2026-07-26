use crate::config::MergedConfig;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use walkdir::{DirEntry, WalkDir};

#[derive(Debug, Clone, Default)]
pub struct TectonicOptions {
    pub arguments: Vec<String>,
    pub search_paths: Vec<PathBuf>,
}

pub fn build_options(config: &MergedConfig, project_path: &Path) -> TectonicOptions {
    let search_paths = search_paths(config, project_path);
    let mut arguments = Vec::new();

    if let Some(bundle) = config
        .tectonic_bundle
        .as_deref()
        .map(str::trim)
        .filter(|bundle| !bundle.is_empty())
    {
        arguments.push(format!(
            "--bundle={}",
            resolve_bundle(project_path, bundle).to_string_lossy()
        ));
    }
    if config.tectonic_only_cached {
        arguments.push("--only-cached".to_string());
    }
    for path in &search_paths {
        arguments.push(format!("-Zsearch-path={}", path.to_string_lossy()));
    }
    if config.tectonic_shell_escape {
        let working_directory = project_path
            .canonicalize()
            .unwrap_or_else(|_| project_path.to_path_buf());
        arguments.push(format!(
            "-Zshell-escape-cwd={}",
            working_directory.to_string_lossy()
        ));
    }

    TectonicOptions {
        arguments,
        search_paths,
    }
}

pub fn makefile_rules_argument(path: &Path) -> String {
    format!("--makefile-rules={}", path.to_string_lossy())
}

pub fn search_paths(config: &MergedConfig, project_path: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for configured in &config.tectonic_search_paths {
        push_configured_root(&mut roots, project_path, configured);
    }
    for configured in [
        config.texinputs.as_deref(),
        config.bibinputs.as_deref(),
        config.texmfhome.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        for value in split_path_list(configured) {
            push_configured_root(&mut roots, project_path, value);
        }
    }
    for default in [project_path.join("tex"), project_path.join("biblio")] {
        push_unique_path(&mut roots, default);
    }
    if let Some(library) = config.lib_path.as_deref() {
        push_unique_path(&mut roots, PathBuf::from(library).join("texmf"));
    }

    let mut directories = Vec::new();
    let mut seen = BTreeSet::new();
    for root in roots {
        if !root.is_dir() {
            continue;
        }
        // Follow an explicitly configured symlink root, while keeping nested
        // symlink traversal disabled below. This supports shared texmf roots
        // without allowing an unexpected recursive walk outside them.
        let root = root.canonicalize().unwrap_or(root);
        for entry in WalkDir::new(&root)
            .follow_links(false)
            .sort_by_file_name()
            .into_iter()
            .filter_entry(|entry| should_visit(entry, &root))
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
        {
            let Some(parent) = entry.path().parent() else {
                continue;
            };
            let resolved = parent
                .canonicalize()
                .unwrap_or_else(|_| parent.to_path_buf());
            if seen.insert(resolved.clone()) {
                directories.push(resolved);
            }
        }
    }
    directories
}

fn should_visit(entry: &DirEntry, root: &Path) -> bool {
    if entry.path() == root {
        return true;
    }
    let name = entry.file_name().to_string_lossy();
    !matches!(
        name.as_ref(),
        ".git" | ".omnidoc-cache" | "build" | "node_modules" | "target"
    )
}

fn push_configured_root(roots: &mut Vec<PathBuf>, project_path: &Path, configured: &str) {
    let configured = configured.trim();
    if configured.is_empty() {
        return;
    }
    let expanded = expand_home(configured);
    let trimmed = expanded
        .trim_end_matches("//")
        .trim_end_matches("\\\\")
        .trim();
    if trimmed.is_empty() {
        return;
    }
    let path = PathBuf::from(trimmed);
    let path = if path.is_absolute() {
        path
    } else {
        project_path.join(path)
    };
    push_unique_path(roots, path);
}

fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if paths.iter().any(|existing| existing == &path) {
        return;
    }
    paths.push(path);
}

fn split_path_list(value: &str) -> Vec<&str> {
    #[cfg(windows)]
    let separator = ';';
    #[cfg(not(windows))]
    let separator = ':';
    value.split(separator).collect()
}

pub(crate) fn resolve_bundle(project_path: &Path, bundle: &str) -> PathBuf {
    if bundle.contains("://") {
        return PathBuf::from(bundle);
    }
    let expanded = PathBuf::from(expand_home(bundle));
    let resolved = if expanded.is_absolute() {
        expanded
    } else {
        project_path.join(expanded)
    };
    resolved.canonicalize().unwrap_or(resolved)
}

pub(crate) fn expand_home(value: &str) -> String {
    let Some(home) = dirs::home_dir() else {
        return value.to_string();
    };
    let home = home.to_string_lossy();
    let mut expanded = value.replace("$ENV{HOME}", &home).replace("$HOME", &home);
    if expanded == "~" || expanded.starts_with("~/") || expanded.starts_with("~\\") {
        expanded = expanded.replacen('~', &home, 1);
    }
    expanded
}

#[cfg(test)]
mod tests {
    use super::{build_options, search_paths};
    use crate::config::MergedConfig;
    use std::fs;

    #[test]
    fn discovers_leaf_directories_in_project_and_library_tex_trees() {
        let project = tempfile::tempdir().expect("project");
        let library = tempfile::tempdir().expect("library");
        let project_package = project.path().join("tex/local/probe.sty");
        let library_package = library.path().join("texmf/tex/common/theme.sty");
        fs::create_dir_all(project_package.parent().expect("project package parent"))
            .expect("project tree");
        fs::create_dir_all(library_package.parent().expect("library package parent"))
            .expect("library tree");
        fs::write(&project_package, "project").expect("project package");
        fs::write(&library_package, "library").expect("library package");
        let config = MergedConfig {
            lib_path: Some(library.path().to_string_lossy().to_string()),
            texinputs: Some("./tex//:".to_string()),
            ..Default::default()
        };

        let paths = search_paths(&config, project.path());

        assert!(paths.iter().any(|path| path.ends_with("tex/local")));
        assert!(paths.iter().any(|path| path.ends_with("texmf/tex/common")));
    }

    #[test]
    fn renders_offline_bundle_and_shell_escape_arguments_explicitly() {
        let project = tempfile::tempdir().expect("project");
        let bundle = project.path().join("bundle.tar");
        fs::write(&bundle, "bundle").expect("bundle");
        let config = MergedConfig {
            tectonic_bundle: Some("bundle.tar".to_string()),
            tectonic_only_cached: true,
            tectonic_shell_escape: true,
            ..Default::default()
        };

        let options = build_options(&config, project.path()).arguments;

        assert!(options
            .iter()
            .any(|option| option == &format!("--bundle={}", bundle.display())));
        assert!(options.iter().any(|option| option == "--only-cached"));
        assert!(options
            .iter()
            .any(|option| option.starts_with("-Zshell-escape-cwd=")));
    }

    #[cfg(unix)]
    #[test]
    fn traverses_an_explicit_symlink_search_root() {
        use std::os::unix::fs::symlink;

        let project = tempfile::tempdir().expect("project");
        let shared = tempfile::tempdir().expect("shared texmf");
        let package = shared.path().join("tex/local/probe.sty");
        fs::create_dir_all(package.parent().expect("package parent")).expect("package tree");
        fs::write(&package, "shared").expect("package");
        symlink(shared.path(), project.path().join("shared-texmf")).expect("texmf symlink");
        let config = MergedConfig {
            tectonic_search_paths: vec!["shared-texmf".to_string()],
            ..Default::default()
        };

        let paths = search_paths(&config, project.path());

        assert!(paths.iter().any(|path| path.ends_with("tex/local")));
    }
}
