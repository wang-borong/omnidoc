use crate::config::global::GlobalConfig;
use crate::constants::git_refs;
use crate::doc::templates::get_latexmkrc_template;
use crate::error::{OmniDocError, Result};
use crate::git::{git_clone, git_pull};
use crate::utils::fs;
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
        // 从配置获取库 URL，如果没有配置则使用默认值
        let global_config = GlobalConfig::load()?;
        let lib_url = global_config
            .get_config()
            .and_then(|c| c.lib.lib.as_ref())
            .and_then(|l| l.url.clone())
            .unwrap_or_else(|| "https://github.com/wang-borong/omnidoc-libs".to_string());

        git_clone(&lib_url, &olib, true).map_err(|e| OmniDocError::Git(e))?;
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
    if !fs::exists(&latexmkrc) {
        fs::create_dir_all(&latexmkrc)?;
    }

    latexmkrc.push("latexmkrc");
    if !fs::exists(&latexmkrc) {
        let latexmkrc_content = get_latexmkrc_template();
        fs::write(&latexmkrc, latexmkrc_content.as_bytes())?;
    }

    Ok(())
}
