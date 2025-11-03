use super::project::Doc;
use crate::cmd::do_cmd;
use crate::constants::{commands, file_names, paths};
use crate::error::{OmniDocError, Result};
use std::path::Path;

impl<'a> Doc<'a> {
    /// Open the built document
    pub fn open_doc(&self) -> Result<()> {
        // Check if the path is a valid omnidoc project
        use super::utils::is_omnidoc_project;
        if !is_omnidoc_project() {
            return Err(OmniDocError::NotOmniDocProject(
                "Current directory is not an omnidoc project".to_string(),
            ));
        }

        // Determine output directory
        let outdir = self.envs["outdir"]
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or(paths::DEFAULT_BUILD_DIR);

        let docname = self.get_docname();
        let doc_path_str = format!("{}/{}.{}", outdir, &docname, file_names::PDF_EXTENSION);
        let doc_path = Path::new(&doc_path_str);

        if !doc_path.exists() {
            return Err(OmniDocError::Project(format!(
                "The document '{}' does not exist",
                doc_path_str
            )));
        }

        let doc_path_str_for_cmd = doc_path.to_str().ok_or_else(|| {
            OmniDocError::Other("Failed to convert document path to string".to_string())
        })?;

        do_cmd(commands::XDG_OPEN, &[doc_path_str_for_cmd], true)
    }
}
