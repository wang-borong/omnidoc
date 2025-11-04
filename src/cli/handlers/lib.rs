use crate::constants::git_refs;
use crate::error::{OmniDocError, Result};
use crate::fs;
use crate::git::{git_clone, git_pull};
use console::style;
use dirs::{config_local_dir, data_local_dir};

/// Handle the 'lib' command
pub fn handle_lib(update: bool) -> Result<()> {
    let dld = data_local_dir()
        .ok_or_else(|| OmniDocError::Other("Local data directory not found".to_string()))?;
    let olib = dld.join("omnidoc");

    if update {
        git_pull(&olib, git_refs::ORIGIN, git_refs::MAIN_BRANCH)
            .map_err(|e| OmniDocError::Git(e))?;
        println!(
            "{} {} '{}'",
            style("✔").green().bold(),
            style("OmniDoc library updated in").green().bold(),
            olib.display()
        );
    } else {
        git_clone("https://github.com/wang-borong/omnidoc-libs", &olib, true)
            .map_err(|e| OmniDocError::Git(e))?;
        println!(
            "{} {} '{}'",
            style("✔").green().bold(),
            style("OmniDoc library installed in").green().bold(),
            olib.display()
        );
    }

    let mut latexmkrc = config_local_dir().ok_or_else(|| {
        OmniDocError::Other("Local configuration directory not found".to_string())
    })?;

    latexmkrc.push("latexmk");
    if !latexmkrc.exists() {
        fs::create_dir_all(&latexmkrc).map_err(|e| OmniDocError::Io(e))?;
    }

    latexmkrc.push("latexmkrc");
    if !latexmkrc.exists() {
        fs::copy_from_lib(crate::constants::paths_internal::REPO_LATEXMKRC, &latexmkrc)
            .map_err(|e| OmniDocError::Io(e))?;
    }

    Ok(())
}
