use crate::config::ConfigParser;
use crate::error::{OmniDocError, Result};
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

/// Unified context for command execution, managing configuration and environment
pub struct CommandContext {
    pub config: ConfigParser,
    pub envs: HashMap<&'static str, Option<String>>,
    pub original_dir: PathBuf,
    pub project_dir: Option<PathBuf>,
}

impl CommandContext {
    /// Create a new CommandContext with default configuration
    pub fn new() -> Result<Self> {
        let config = ConfigParser::default()
            .map_err(|e| OmniDocError::Config(format!("Failed to load config: {}", e)))?;

        let envs = config
            .get_envs()
            .map_err(|e| OmniDocError::Config(format!("Failed to get envs: {}", e)))?;

        let original_dir = env::current_dir().map_err(|e| OmniDocError::Io(e))?;

        Ok(Self {
            config,
            envs,
            original_dir,
            project_dir: None,
        })
    }

    /// Set the project path for this context
    pub fn with_project_path(mut self, path: Option<String>) -> Result<Self> {
        if let Some(path_str) = path {
            let project_path = PathBuf::from(&path_str);
            if !project_path.exists() {
                return Err(OmniDocError::Project(format!(
                    "Path does not exist: {}",
                    path_str
                )));
            }
            self.project_dir = Some(project_path);
        }
        Ok(self)
    }

    /// Enter the project directory
    pub fn enter_project(&mut self) -> Result<()> {
        if let Some(ref project_dir) = self.project_dir {
            env::set_current_dir(project_dir).map_err(|e| OmniDocError::Io(e))?;
        }
        Ok(())
    }

    /// Restore the original directory
    pub fn restore(&mut self) -> Result<()> {
        env::set_current_dir(&self.original_dir).map_err(|e| OmniDocError::Io(e))?;
        Ok(())
    }

    /// Get the author name from config, with fallback
    pub fn get_author(&self, override_author: Option<String>) -> String {
        override_author.unwrap_or_else(|| {
            self.config
                .get_author_name()
                .unwrap_or_else(|_| "Someone".to_string())
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
        let original_dir = env::current_dir().map_err(|e| OmniDocError::Io(e))?;

        let project_dir = match path {
            Some(ref p) => {
                let pb = PathBuf::from(p);
                if !pb.exists() {
                    return Err(OmniDocError::Project(format!("Path does not exist: {}", p)));
                }
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
        env::set_current_dir(&self.project_dir).map_err(|e| OmniDocError::Io(e))?;
        Ok(())
    }

    /// Restore the original directory
    pub fn restore(&mut self) -> Result<()> {
        env::set_current_dir(&self.original_dir).map_err(|e| OmniDocError::Io(e))?;
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
