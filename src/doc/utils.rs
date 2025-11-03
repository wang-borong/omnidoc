use crate::constants::{file_names, paths, paths_internal};
use std::env;
use std::path::Path;

/// Check if current directory is an omnidoc project
pub fn is_omnidoc_project() -> bool {
    let check_paths = [
        format!("{}/{}", paths_internal::CURRENT_DIR, paths::MAIN_MD),
        format!("{}/{}", paths_internal::CURRENT_DIR, paths::MAIN_TEX),
        format!("{}/{}", paths_internal::PARENT_DIR, paths::MAIN_MD),
        format!("{}/{}", paths_internal::PARENT_DIR, paths::MAIN_TEX),
        format!("{}/{}", paths_internal::PARENT_PARENT_DIR, paths::MAIN_MD),
        format!("{}/{}", paths_internal::PARENT_PARENT_DIR, paths::MAIN_TEX),
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
