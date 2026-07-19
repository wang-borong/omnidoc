use crate::config::schema::PathConfig;
use crate::constants::paths;
use std::env;
use std::path::Path;

/// Check if current directory is an omnidoc project
/// This function works before configuration is loaded, so it uses default paths
pub fn is_omnidoc_project() -> bool {
    is_omnidoc_project_with_paths(None)
}

/// Check if current directory is an omnidoc project with custom paths
pub fn is_omnidoc_project_with_paths(paths: Option<&PathConfig>) -> bool {
    let default_paths = PathConfig::new();
    let paths = paths.unwrap_or(&default_paths);
    let Ok(current) = env::current_dir() else {
        return false;
    };
    let Some(root) = locate_project_root(&current, paths) else {
        return false;
    };
    let _ = env::set_current_dir(root);
    true
}

fn locate_project_root(start: &Path, paths_config: &PathConfig) -> Option<std::path::PathBuf> {
    start.ancestors().take(3).find_map(|directory| {
        [
            paths::PROJECT_CONFIG,
            paths_config.main_md.as_str(),
            paths_config.main_tex.as_str(),
        ]
        .iter()
        .any(|name| directory.join(name).is_file())
        .then(|| directory.to_path_buf())
    })
}

#[cfg(test)]
mod tests {
    use super::locate_project_root;
    use crate::config::schema::PathConfig;
    use std::fs;

    #[test]
    fn recognizes_configured_projects_without_default_entry_names() {
        let project = tempfile::tempdir().expect("project");
        let nested = project.path().join("chapters/drafts");
        fs::create_dir_all(&nested).expect("nested directory");
        fs::write(
            project.path().join(".omnidoc.toml"),
            "[project]\nentry = 'article.md'\n",
        )
        .expect("project config");
        fs::write(project.path().join("article.md"), "# Article\n").expect("entry");

        assert_eq!(
            locate_project_root(&nested, &PathConfig::new()),
            Some(project.path().to_path_buf())
        );
    }
}
