use crate::doctype::DocumentTypeRegistry;
use crate::error::{OmniDocError, Result};
use crate::rl::DTRL;
use std::env;
use std::fs;
use std::path::Path;

/// Print all supported document types
pub fn print_doctypes() {
    println!("Current supported document types:");
    println!("{}", DocumentTypeRegistry::list_display());
}

/// Get document type from readline with cleanup on error
pub fn get_doctype_from_readline<O, N>(orig_path: O, path: N) -> Result<String>
where
    O: AsRef<Path>,
    N: AsRef<Path>,
{
    let mut dtrl = DTRL::new().map_err(|e| {
        let _ = env::set_current_dir(orig_path.as_ref());
        let _ = fs::remove_dir_all(path.as_ref());
        OmniDocError::Other(format!("Create DTRL failed ({})", e))
    })?;

    loop {
        let doctype = dtrl.readline().map_err(|e| {
            let _ = env::set_current_dir(orig_path.as_ref());
            let _ = fs::remove_dir_all(path.as_ref());
            OmniDocError::Other(format!("Get the input line failed ({})", e))
        })?;

        if &doctype == "list" || &doctype == "ls" {
            print_doctypes();
        } else {
            return Ok(doctype);
        }
    }
}

/// Check if omnidoc library exists
pub fn omnidoc_lib_exists() -> bool {
    let local_data_dir = match dirs::data_local_dir() {
        Some(dir) => dir,
        None => return false,
    };
    let omnidoc_lib_dir = local_data_dir.join("omnidoc");
    omnidoc_lib_dir.exists()
}
