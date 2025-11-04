use crate::cli::handlers::common::{check_omnidoc_project, create_build_service};
use crate::config::CliOverrides;
use crate::error::Result;
use crate::utils::path;

/// Handle the 'clean' command
pub fn handle_clean(path: Option<String>, distclean: bool) -> Result<()> {
    let project_path = path::determine_project_path(path)?;
    check_omnidoc_project(&project_path)?;

    let build_service = create_build_service(Some(&project_path), CliOverrides::new())?;

    // 执行清理
    build_service.clean(&project_path, distclean)?;

    Ok(())
}
