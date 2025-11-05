use crate::config::schema::ConfigSchema;
use crate::error::{OmniDocError, Result};
use std::path::{Path, PathBuf};

const PROJECT_CONFIG_FILE: &str = ".omnidoc.toml";

/// 项目配置管理器
pub struct ProjectConfig {
    path: PathBuf,
    config: Option<ConfigSchema>,
}

impl ProjectConfig {
    /// 从当前目录或指定路径加载项目配置
    pub fn load_from_path(path: Option<&Path>) -> Result<Option<Self>> {
        let search_paths = if let Some(p) = path {
            vec![p.to_path_buf()]
        } else {
            // 从当前目录向上查找
            let mut paths = Vec::new();
            let mut current = std::env::current_dir().map_err(|e| OmniDocError::Io(e))?;

            for _ in 0..10 {
                // 限制搜索深度
                paths.push(current.clone());
                if !current.pop() {
                    break;
                }
            }
            paths
        };

        for search_path in search_paths {
            let config_path = search_path.join(PROJECT_CONFIG_FILE);
            if config_path.exists() {
                return Ok(Some(Self::from_file(&config_path)?));
            }
        }

        Ok(None)
    }

    /// 从文件加载配置
    pub fn from_file(path: &Path) -> Result<Self> {
        use crate::utils::fs;
        let content = fs::read_to_string(path)?;

        let config: ConfigSchema = toml::from_str(&content)
            .map_err(|e| OmniDocError::Config(format!("Failed to parse project config: {}", e)))?;

        Ok(Self {
            path: path.to_path_buf(),
            config: Some(config),
        })
    }

    /// 创建默认项目配置
    pub fn create_default(
        path: &Path,
        entry: Option<&str>,
        from: Option<&str>,
        to: Option<&str>,
        target: Option<&str>,
    ) -> Result<Self> {
        use crate::config::schema::{
            BuildConfig, BuildSection, ConfigSchema, FigureConfig, FigureSection,
            ProjectConfig as ProjectConfigSchema, ProjectSection,
        };

        let mut config = ConfigSchema::default();

        // 设置项目配置
        let project_section = ProjectSection {
            entry: entry.map(|s| s.to_string()),
            from: from.map(|s| s.to_string()),
            to: to.map(|s| s.to_string()),
            target: target.map(|s| s.to_string()),
        };
        config.project = Some(ProjectConfigSchema {
            project: Some(project_section),
        });

        // 设置构建配置
        config.build = Some(BuildConfig {
            build: Some(BuildSection {
                outdir: Some("build".to_string()),
                metadata_file: None,
                verbose: Some(false),
            }),
        });

        // 设置图片配置
        config.figure = Some(FigureConfig {
            figure: Some(FigureSection {
                paths: Some(vec!["drawio".to_string(), "dac".to_string()]),
                output: Some("figures".to_string()),
            }),
        });

        let config_path = path.join(PROJECT_CONFIG_FILE);
        let toml_content = toml::to_string_pretty(&config)
            .map_err(|e| OmniDocError::Config(format!("Failed to serialize config: {}", e)))?;

        use crate::utils::fs;
        fs::write(&config_path, toml_content.as_bytes())?;

        Ok(Self {
            path: config_path,
            config: Some(config),
        })
    }

    /// 获取配置
    pub fn get_config(&self) -> Option<&ConfigSchema> {
        self.config.as_ref()
    }

    /// 获取配置路径
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 检查配置是否存在
    pub fn exists(path: &Path) -> bool {
        path.join(PROJECT_CONFIG_FILE).exists()
    }
}
