use crate::config::schema::ConfigSchema;
use crate::constants::config as config_consts;
use crate::error::{OmniDocError, Result};
use console::style;
use dirs::{config_local_dir, data_local_dir};
use std::env::var;
use std::fs;
use std::path::PathBuf;

/// 全局配置管理器（~/.config/omnidoc.toml）
pub struct GlobalConfig {
    path: PathBuf,
    config: Option<ConfigSchema>,
}

impl GlobalConfig {
    /// 加载全局配置
    pub fn load() -> Result<Self> {
        let config_local_dir = match config_local_dir() {
            None => {
                let home_path = var("HOME").map_err(|_| {
                    OmniDocError::Config("HOME environment variable not found".to_string())
                })?;
                let mut conf_dir = PathBuf::from(home_path);
                conf_dir.push(config_consts::CONFIG_DIR);
                let _ = fs::create_dir_all(&conf_dir);
                conf_dir
            }
            Some(cld) => cld,
        };

        let config_file = config_local_dir.join(config_consts::OMNIDOC_CONFIG_FILE);
        
        // 如果配置文件不存在，创建默认配置
        if !config_file.exists() {
            Self::create_default(&config_file)?;
            println!(
                "{} The '{}' configuration file was created in '{}'.\n    You can modify it to set your author name.",
                style("ℹ").cyan().bold(),
                config_consts::OMNIDOC_CONFIG_FILE,
                config_local_dir.display()
            );
        }

        let content = fs::read_to_string(&config_file)
            .map_err(|e| OmniDocError::Config(format!("Failed to read global config: {}", e)))?;
        
        let config: ConfigSchema = toml::from_str(&content)
            .map_err(|e| OmniDocError::Config(format!("Failed to parse global config: {}", e)))?;

        Ok(Self {
            path: config_file,
            config: Some(config),
        })
    }

    /// 创建默认全局配置
    pub fn create_default(path: &PathBuf) -> Result<()> {
        use crate::config::schema::*;
        
        let mut config = ConfigSchema::default();
        
        // 设置默认作者
        config.author = AuthorConfig {
            author: Some(AuthorSection {
                name: Some(config_consts::UNKNOWN_AUTHOR.to_string()),
            }),
        };

        // 设置默认库路径
        let dld = data_local_dir()
            .ok_or_else(|| OmniDocError::Config("Local data directory not found".to_string()))?;
        let olib = dld.join("omnidoc");
        let lib_path_str = olib.to_str().ok_or_else(|| {
            OmniDocError::Config("Library path contains invalid UTF-8".to_string())
        })?;
        
        config.lib = LibConfig {
            lib: Some(LibSection {
                path: Some(lib_path_str.to_string()),
            }),
        };

        // 设置默认环境变量
        config.env = EnvConfig {
            env: Some(EnvSection {
                outdir: Some("build".to_string()),
                texmfhome: Some(r"$ENV{HOME}/.local/share/omnidoc/texmf//:".to_string()),
                bibinputs: Some(r"./biblio//:".to_string()),
                texinputs: Some(r"./tex//:".to_string()),
            }),
        };

        let toml_content = toml::to_string_pretty(&config)
            .map_err(|e| OmniDocError::Config(format!("Failed to serialize config: {}", e)))?;
        
        fs::write(path, toml_content)
            .map_err(|e| OmniDocError::Io(e))?;

        Ok(())
    }

    /// 获取配置
    pub fn get_config(&self) -> Option<&ConfigSchema> {
        self.config.as_ref()
    }

    /// 获取配置路径
    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

