use crate::config::schema::ConfigSchema;
use crate::constants::config as config_consts;
use crate::error::{OmniDocError, Result};
use crate::utils::directories::{config_local_dir, data_local_dir};
use crate::utils::fs;
use std::env::var;
use std::path::{Path, PathBuf};

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

        let config = if fs::exists(&config_file) {
            let content = fs::read_to_string(&config_file)?;
            toml::from_str(&content).map_err(|e| {
                OmniDocError::Config(format!("Failed to parse global config: {}", e))
            })?
        } else {
            Self::default_schema()?
        };

        Ok(Self {
            path: config_file,
            config: Some(config),
        })
    }

    /// 创建默认全局配置
    pub fn create_default(path: &Path) -> Result<()> {
        let config = Self::default_schema()?;
        let toml_content = toml::to_string_pretty(&config)
            .map_err(|e| OmniDocError::Config(format!("Failed to serialize config: {}", e)))?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::atomic_write(path, toml_content.as_bytes())?;
        Ok(())
    }

    /// Build the effective first-run defaults without writing to disk.
    pub fn default_schema() -> Result<ConfigSchema> {
        use crate::config::schema::*;

        let mut config = ConfigSchema {
            author: AuthorConfig {
                author: Some(AuthorSection {
                    name: Some(config_consts::UNKNOWN_AUTHOR.to_string()),
                }),
            },
            ..Default::default()
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

        // 设置默认环境变量（避免使用 $HOME 占位符，直接写实际路径）
        let texmf_path = olib.join("texmf").to_string_lossy().to_string() + "//:";
        config.env = EnvConfig {
            env: Some(EnvSection {
                outdir: Some("build".to_string()),
                texmfhome: Some(texmf_path),
                bibinputs: Some(r"./biblio//:".to_string()),
                texinputs: Some(r"./tex//:".to_string()),
            }),
        };

        Ok(config)
    }

    /// 获取配置
    pub fn get_config(&self) -> Option<&ConfigSchema> {
        self.config.as_ref()
    }

    /// 获取配置路径
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn exists(&self) -> bool {
        self.path.is_file()
    }
}
