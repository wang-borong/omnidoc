use crate::config::schema::PathConfig;
use crate::constants::paths_internal;
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

    let check_paths = [
        format!("{}/{}", paths_internal::CURRENT_DIR, paths.main_md),
        format!("{}/{}", paths_internal::CURRENT_DIR, paths.main_tex),
        format!("{}/{}", paths_internal::PARENT_DIR, paths.main_md),
        format!("{}/{}", paths_internal::PARENT_DIR, paths.main_tex),
        format!("{}/{}", paths_internal::PARENT_PARENT_DIR, paths.main_md),
        format!("{}/{}", paths_internal::PARENT_PARENT_DIR, paths.main_tex),
    ];
    for p in &check_paths {
        let path = Path::new(p.as_str());
        if path.exists() {
            match path.parent() {
                Some(parent) => {
                    if parent.to_str().unwrap_or("") != "" {
                        let _ = env::set_current_dir(parent);
                    }
                }
                None => {}
            }
            return true;
        }
    }
    false
}
