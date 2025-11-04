pub mod cli;
pub mod global;
pub mod manager;
pub mod project;
pub mod schema;

// 向后兼容：保留原有的 ConfigParser
pub use cli::CliOverrides;
pub use global::GlobalConfig;
pub use manager::{ConfigManager, MergedConfig};
pub use project::ProjectConfig;
pub use schema::*;

// 向后兼容的 ConfigParser
use crate::constants::config as config_consts;
use crate::error::{OmniDocError, Result};
use dirs::config_local_dir;
use serde::Deserialize;
use std::collections::HashMap;
use std::env::set_var as env_set_var;
use std::fs;
use std::path::PathBuf;

#[derive(Deserialize, Debug)]
struct DownloadConfig {
    url: String,
    filename: String,
}

#[derive(Deserialize, Debug)]
struct Author {
    name: Option<String>,
}

#[derive(Deserialize, Debug)]
struct Lib {
    path: Option<String>,
}

#[derive(Deserialize, Debug)]
struct Env {
    outdir: Option<String>,
    texmfhome: Option<String>,
    bibinputs: Option<String>,
    texinputs: Option<String>,
}

#[derive(Deserialize, Debug)]
struct Config {
    download: Option<Vec<DownloadConfig>>,
    author: Author,
    lib: Lib,
    env: Env,
    template_dir: Option<String>,
}

/// 向后兼容的配置解析器
/// 新代码应该使用 ConfigManager
pub struct ConfigParser {
    config: Option<Config>,
    path: PathBuf,
}

impl ConfigParser {
    pub fn default() -> Result<Self> {
        let global = GlobalConfig::load()?;
        let config_file = global.path();

        // 转换为旧格式进行兼容
        let config_cont = fs::read_to_string(config_file)
            .map_err(|e| OmniDocError::Config(format!("Failed to read config file: {}", e)))?;
        let config: Config = toml::from_str(&config_cont)
            .map_err(|e| OmniDocError::Config(format!("Failed to parse config file: {}", e)))?;

        Ok(Self {
            config: Some(config),
            path: config_file.clone(),
        })
    }

    pub fn parse(&mut self) -> Result<()> {
        if !self.path.exists() {
            return Err(OmniDocError::ConfigNotFound(format!(
                "No OmniDoc config file found at {}. Please create it using 'omnidoc config'",
                self.path.display()
            )));
        }

        let config_cont = fs::read_to_string(&self.path)
            .map_err(|e| OmniDocError::Config(format!("Failed to read config file: {}", e)))?;
        let config: Config = toml::from_str(&config_cont)
            .map_err(|e| OmniDocError::Config(format!("Failed to parse config file: {}", e)))?;

        self.config = Some(config);
        Ok(())
    }

    pub fn get_downloads(&self) -> Result<HashMap<String, String>> {
        let config = self
            .config
            .as_ref()
            .ok_or_else(|| OmniDocError::Config("Configuration not loaded".to_string()))?;

        let mut downloads = HashMap::new();
        if let Some(download_list) = &config.download {
            for download in download_list {
                downloads.insert(download.url.clone(), download.filename.clone());
            }
        }
        Ok(downloads)
    }

    pub fn get_author_name(&self) -> Result<String> {
        let config = self
            .config
            .as_ref()
            .ok_or_else(|| OmniDocError::Config("Configuration not loaded".to_string()))?;

        match &config.author.name {
            Some(author) => Ok(author.to_owned()),
            None => Err(OmniDocError::Config(
                "No author name configured".to_string(),
            )),
        }
    }

    pub fn get_omnidoc_lib(&self) -> Result<String> {
        let config = self
            .config
            .as_ref()
            .ok_or_else(|| OmniDocError::Config("Configuration not loaded".to_string()))?;

        match &config.lib.path {
            Some(lib_path) => Ok(lib_path.to_owned()),
            None => Err(OmniDocError::Config(
                "No OmniDoc library configured".to_string(),
            )),
        }
    }

    pub fn get_template_dir(&self) -> Option<String> {
        self.config.as_ref().and_then(|c| c.template_dir.clone())
    }

    pub fn get_envs(&self) -> Result<HashMap<&'static str, Option<String>>> {
        let mut envs: HashMap<&'static str, Option<String>> = HashMap::new();
        let config = self
            .config
            .as_ref()
            .ok_or_else(|| OmniDocError::Config("Configuration not loaded".to_string()))?;

        if let Some(outdir) = &config.env.outdir {
            envs.insert("outdir", Some(outdir.to_owned()));
        } else {
            envs.insert("outdir", None);
        }

        if let Some(texmfhome) = &config.env.texmfhome {
            envs.insert("texmfhome", Some(texmfhome.to_owned()));
        } else {
            envs.insert("texmfhome", None);
        }

        if let Some(texinputs) = &config.env.texinputs {
            envs.insert("texinputs", Some(texinputs.to_owned()));
        } else {
            envs.insert("texinputs", None);
        }

        if let Some(bibinputs) = &config.env.bibinputs {
            envs.insert("bibinputs", Some(bibinputs.to_owned()));
        } else {
            envs.insert("bibinputs", None);
        }

        Ok(envs)
    }

    pub fn gen(
        author: String,
        lib: Option<String>,
        outdir: Option<String>,
        texmfhome: Option<String>,
        bibinputs: Option<String>,
        texinputs: Option<String>,
        force: bool,
    ) -> Result<()> {
        GlobalConfig::create_default(&config_local_dir()
            .ok_or_else(|| OmniDocError::Config("Local config directory not found".to_string()))?
            .join(config_consts::OMNIDOC_CONFIG_FILE))?;
        Ok(())
    }

    pub fn setup_env(&self) -> Result<()> {
        let config = self
            .config
            .as_ref()
            .ok_or_else(|| OmniDocError::Config("Configuration not loaded".to_string()))?;

        if let Some(texmfhome) = &config.env.texmfhome {
            env_set_var("TEXMFHOME", texmfhome);
        }
        if let Some(bibinputs) = &config.env.bibinputs {
            env_set_var("BIBINPUTS", bibinputs);
        }
        if let Some(texinputs) = &config.env.texinputs {
            env_set_var("TEXINPUTS", texinputs);
        }

        Ok(())
    }
}

