use crate::config::{CliOverrides, ConfigManager, MergedConfig};
use crate::error::Result;
use crate::utils::{error, path};
use std::collections::HashMap;
use std::path::PathBuf;

/// Convert MergedConfig to Doc's envs HashMap format
fn merged_config_to_envs(merged_config: &MergedConfig) -> HashMap<&'static str, Option<String>> {
    let mut envs = HashMap::new();
    envs.insert("outdir", merged_config.outdir.clone());
    envs.insert("texmfhome", merged_config.texmfhome.clone());
    envs.insert("texinputs", merged_config.texinputs.clone());
    envs.insert("bibinputs", merged_config.bibinputs.clone());
    envs
}

/// Unified context for command execution, managing configuration and environment
pub struct CommandContext {
    pub config_manager: ConfigManager,
    pub merged_config: MergedConfig,
    pub envs: HashMap<&'static str, Option<String>>,
    pub original_dir: PathBuf,
    pub project_dir: Option<PathBuf>,
}

impl CommandContext {
    /// Create a new CommandContext with default configuration
    pub fn new() -> Result<Self> {
        let config_manager = error::config_err(
            ConfigManager::new(None, CliOverrides::new()),
            "Failed to load config",
        )?;

        let merged_config = config_manager.get_merged().clone();
        let envs = merged_config_to_envs(&merged_config);

        let original_dir = path::current_dir()?;

        Ok(Self {
            config_manager,
            merged_config,
            envs,
            original_dir,
            project_dir: None,
        })
    }

    /// Set the project path for this context
    pub fn with_project_path(mut self, path: Option<String>) -> Result<Self> {
        if let Some(path_str) = path {
            let project_path = path::from_str(&path_str);
            path::validate_project_path(&project_path)?;

            // Reload config with project path
            let project_path_ref = Some(project_path.as_path());
            self.config_manager = error::config_err(
                ConfigManager::new(project_path_ref, CliOverrides::new()),
                "Failed to load config",
            )?;
            self.merged_config = self.config_manager.get_merged().clone();
            self.envs = merged_config_to_envs(&self.merged_config);

            self.project_dir = Some(project_path);
        }
        Ok(self)
    }

    /// Enter the project directory
    pub fn enter_project(&mut self) -> Result<()> {
        if let Some(ref project_dir) = self.project_dir {
            path::set_current_dir(project_dir)?;
        }
        Ok(())
    }

    /// Restore the original directory
    pub fn restore(&mut self) -> Result<()> {
        path::set_current_dir(&self.original_dir)?;
        Ok(())
    }

    /// Get the author name from config, with fallback
    pub fn get_author(&self, override_author: Option<String>) -> String {
        override_author.unwrap_or_else(|| {
            self.merged_config
                .author
                .clone()
                .unwrap_or_else(|| "Someone".to_string())
        })
    }
}

/// Manager for project path operations, handling directory switching and restoration
pub struct ProjectPathManager {
    original_dir: PathBuf,
    project_dir: PathBuf,
}

impl ProjectPathManager {
    /// Create a new ProjectPathManager
    pub fn new(path: Option<String>) -> Result<Self> {
        let original_dir = path::current_dir()?;

        let project_dir = match path {
            Some(ref p) => {
                let pb = path::from_str(p);
                path::validate_project_path(&pb)?;
                pb
            }
            None => original_dir.clone(),
        };

        Ok(Self {
            original_dir,
            project_dir,
        })
    }

    /// Enter the project directory
    pub fn enter_project(&mut self) -> Result<()> {
        path::set_current_dir(&self.project_dir)?;
        Ok(())
    }

    /// Restore the original directory
    pub fn restore(&mut self) -> Result<()> {
        path::set_current_dir(&self.original_dir)?;
        Ok(())
    }

    /// Get the project directory path
    pub fn project_dir(&self) -> &PathBuf {
        &self.project_dir
    }

    /// Get the original directory path
    pub fn original_dir(&self) -> &PathBuf {
        &self.original_dir
    }
}
