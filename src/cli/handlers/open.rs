use crate::cli::handlers::common::{check_omnidoc_project, create_config_manager_default};
use crate::doc::artifacts::{artifact_for_format, primary_output_format};
use crate::doc::open_path;
use crate::error::{OmniDocError, Result};
use crate::utils::path;

/// Handle the 'open' command
pub fn handle_open(path: Option<String>, to: Option<String>, print_path: bool) -> Result<()> {
    let project_path = path::determine_project_root(path)?;
    check_omnidoc_project(&project_path)?;

    let config_manager = create_config_manager_default(Some(project_path.as_path()))?;
    let merged_config = config_manager.get_merged();
    let output = match to {
        Some(output) => output,
        None => primary_output_format(merged_config)?,
    };
    let artifact = artifact_for_format(&project_path, merged_config, &output)?;
    let artifact_path = artifact.path_buf();

    if !artifact.exists {
        return Err(OmniDocError::Project(format!(
            "Build artifact '{}' does not exist. Run `omnidoc build --to {} {}` first.",
            artifact_path.display(),
            artifact.format,
            project_path.display()
        )));
    }

    if print_path {
        println!("{}", artifact_path.display());
        return Ok(());
    }

    open_path(&artifact_path)?;

    Ok(())
}
