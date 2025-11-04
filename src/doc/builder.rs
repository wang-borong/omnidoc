use super::project::Doc;
use crate::cmd::do_cmd;
use crate::constants::{build, commands, paths, paths_internal};
use crate::error::{OmniDocError, Result};
use crate::fs;
use dirs::data_local_dir;
use std::env;
use std::path::Path;
use std::process::Command;

impl<'a> Doc<'a> {
    /// Build the project
    pub fn build_project(&self, verbose: bool) -> Result<()> {
        // Check if the path is a valid omnidoc project
        use super::utils::is_omnidoc_project;
        if !is_omnidoc_project() {
            return Err(OmniDocError::NotOmniDocProject(
                "The current directory is not an OmniDoc project".to_string(),
            ));
        }

        // Preflight: check required external tools
        // We check a minimal set that our build pipeline depends on.
        // If any are missing, log helpful messages and exit gracefully.
        fn command_exists(command_name: &str) -> bool {
            Command::new(command_name).arg("--version").output().is_ok()
        }

        let required_tools = ["make", "pandoc", "pandoc-crossref", "xelatex"];
        let missing: Vec<&str> = required_tools
            .iter()
            .copied()
            .filter(|tool| !command_exists(tool))
            .collect();

        if !missing.is_empty() {
            use console::style;
            eprintln!(
                "{} Missing required external tools:",
                style("✖").red().bold()
            );
            for tool in &missing {
                eprintln!(
                    "  - {} is not installed. Please install '{}' and try again.",
                    style(tool).yellow(),
                    tool
                );
            }
            eprintln!(
                "{} Some templates require pandoc, pandoc-crossref, and a LaTeX engine (e.g., xelatex).",
                style("ℹ").blue()
            );
            return Err(OmniDocError::Other(format!(
                "Missing external tools: {}",
                missing.join(", ")
            )));
        }

        // Create build directory
        let conf_o = &self.envs["outdir"];
        let outdir = match conf_o {
            Some(conf_o) => {
                if !Path::new(&conf_o).exists() {
                    fs::create_dir(&conf_o)?;
                }
                env::set_var("OUTDIR", &conf_o);
                conf_o.clone()
            }
            None => {
                if !Path::new(paths::DEFAULT_BUILD_DIR).exists() {
                    fs::create_dir(paths::DEFAULT_BUILD_DIR)?;
                }
                paths::DEFAULT_BUILD_DIR.to_string()
            }
        };

        // Set environment variables
        for env_key in &["texinputs", "bibinputs", "texmfhome"] {
            if let Some(env_val) = &self.envs[*env_key] {
                env::set_var(env_key.to_uppercase(), env_val);
            }
        }

        // Execute make command
        let docname = self.get_docname();
        let target = format!("{}{}", build::TARGET_PREFIX, &docname);

        let mut topmk = data_local_dir()
            .ok_or_else(|| OmniDocError::Other("Local data directory not found".to_string()))?;
        topmk.push(paths_internal::OMNIDOC_TOOL_TOP_MK);

        let topmk_str = topmk
            .to_str()
            .ok_or_else(|| OmniDocError::Other("Failed to convert path to string".to_string()))?;

        let make_args = if verbose {
            vec!["-f", topmk_str, &target, build::MAKE_VERBOSE_FLAG]
        } else {
            vec!["-f", topmk_str, &target]
        };

        do_cmd(commands::MAKE, &make_args, false)?;
        Ok(())
    }
}
