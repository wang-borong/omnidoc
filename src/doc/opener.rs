use super::project::Doc;
use crate::build::executor::BuildExecutor;
use crate::constants::{file_names, paths};
use crate::error::{OmniDocError, Result};
use crate::utils::fs;
use std::collections::HashMap;
use std::path::Path;

pub fn open_path(path: &Path) -> Result<()> {
    if !fs::is_file(path) {
        return Err(OmniDocError::Project(format!(
            "Document '{}' does not exist",
            path.display()
        )));
    }

    let path_str = path
        .to_str()
        .ok_or_else(|| OmniDocError::Other("Failed to convert path to string".to_string()))?;
    let executor = BuildExecutor::new(HashMap::new());

    #[cfg(target_os = "macos")]
    return executor.spawn_system_cmd("open", &[path_str]);

    #[cfg(target_os = "windows")]
    return executor.spawn_system_cmd("cmd", &["/C", "start", "", path_str]);

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    executor.spawn_system_cmd("xdg-open", &[path_str])
}

impl<'a> Doc<'a> {
    /// Open the built document
    pub fn open_doc(&self) -> Result<()> {
        crate::utils::path::check_omnidoc_project(&self.path)?;

        // Determine output directory
        let outdir = self.envs["outdir"]
            .as_deref()
            .unwrap_or(paths::DEFAULT_BUILD_DIR);

        let docname = self.get_docname();
        let doc_path =
            self.path
                .join(outdir)
                .join(format!("{}.{}", docname, file_names::PDF_EXTENSION));
        open_path(&doc_path)
    }
}
