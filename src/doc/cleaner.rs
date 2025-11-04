use super::project::Doc;
use crate::cmd::do_cmd;
use crate::constants::{build, commands, paths_internal};
use crate::error::{OmniDocError, Result};
use dirs::data_local_dir;
use std::env;

impl<'a> Doc<'a> {
    /// Clean the project
    pub fn clean_project(&self, distclean: bool) -> Result<()> {
        // Check if the path is a valid omnidoc project
        use super::utils::is_omnidoc_project;
        if !is_omnidoc_project() {
            return Err(OmniDocError::NotOmniDocProject(
                "The current directory is not an OmniDoc project".to_string(),
            ));
        }

        // Set OUTDIR environment variable
        if let Some(conf_o) = &self.envs["outdir"] {
            env::set_var("OUTDIR", conf_o);
        }

        // Set other environment variables
        for env_key in &["texinputs", "bibinputs", "texmfhome"] {
            if let Some(env_val) = &self.envs[*env_key] {
                env::set_var(env_key.to_uppercase(), env_val);
            }
        }

        // Execute clean command
        let docname = self.get_docname();
        let target = format!("{}{}", build::TARGET_PREFIX, &docname);

        let mut topmk = data_local_dir()
            .ok_or_else(|| OmniDocError::Other("Local data directory not found".to_string()))?;
        topmk.push(paths_internal::OMNIDOC_TOOL_TOP_MK);

        let topmk_str = topmk
            .to_str()
            .ok_or_else(|| OmniDocError::Other("Failed to convert path to string".to_string()))?;

        let clean_target = if distclean {
            build::DISTCLEAN_TARGET
        } else {
            build::CLEAN_TARGET
        };
        let make_args = vec!["-f", topmk_str, &target, clean_target];

        do_cmd(commands::MAKE, &make_args, false)?;
        Ok(())
    }
}
