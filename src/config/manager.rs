use crate::config::cli::CliOverrides;
use crate::config::global::GlobalConfig;
use crate::config::project::ProjectConfig;
use crate::config::schema::*;
use crate::error::Result;
use std::collections::HashMap;
use std::env;
use std::path::Path;

/// 统一配置管理器
/// 处理配置合并：命令行 > 项目配置 > 全局配置
pub struct ConfigManager {
    global: GlobalConfig,
    project: Option<ProjectConfig>,
    #[allow(dead_code)] // Kept for potential future use or debugging
    cli_overrides: CliOverrides,
    merged: MergedConfig,
}

/// 合并后的配置（最终使用的配置值）
#[derive(Debug, Clone)]
pub struct MergedConfig {
    pub author: Option<String>,
    pub lib_path: Option<String>,
    pub lib_url: Option<String>,
    pub outdir: Option<String>,
    pub texmfhome: Option<String>,
    pub bibinputs: Option<String>,
    pub texinputs: Option<String>,
    pub entry: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub target: Option<String>,
    pub metadata_file: Option<String>,
    pub verbose: bool,
    pub figure_paths: Vec<String>,
    pub figure_output: Option<String>,
    pub pandoc_options: Vec<String>,
    pub pandoc_css: Option<String>,
    pub pandoc_from_format: Option<String>,
    pub pandoc_to_format: Option<String>,
    pub pandoc_lua_filters: Vec<String>,
    pub pandoc_template: Option<String>,
    pub pandoc_data_dir: Option<String>,
    pub pandoc_resource_path: Vec<String>,
    pub pandoc_syntax_highlighting: Option<String>,
    pub pandoc_crossref_yaml: Option<String>,
    pub pandoc_python_path: Option<String>,
    pub pandoc_standalone: bool,
    pub pandoc_embed_resources: bool,
    pub pandoc_lang: Option<String>,
    pub tool_paths: HashMap<String, Option<String>>,
    pub template_dir: Option<String>,
    pub paths: PathConfig,
}

impl ConfigManager {
    /// 创建配置管理器
    pub fn new(project_path: Option<&Path>, cli_overrides: CliOverrides) -> Result<Self> {
        let global = GlobalConfig::load()?;
        let project = ProjectConfig::load_from_path(project_path)?;

        let merged = Self::merge_configs(&global, project.as_ref(), &cli_overrides)?;

        Ok(Self {
            global,
            project,
            cli_overrides,
            merged,
        })
    }

    /// 合并配置
    fn merge_configs(
        global: &GlobalConfig,
        project: Option<&ProjectConfig>,
        cli: &CliOverrides,
    ) -> Result<MergedConfig> {
        let global_config = global.get_config();
        let project_config = project.and_then(|p| p.get_config());

        // 合并作者
        let author = cli
            .author
            .clone()
            .or_else(|| {
                project_config
                    .and_then(|c| c.author.author.as_ref())
                    .and_then(|a| a.name.clone())
            })
            .or_else(|| {
                global_config
                    .and_then(|c| c.author.author.as_ref())
                    .and_then(|a| a.name.clone())
            });

        // 合并库路径
        let lib_path = global_config
            .and_then(|c| c.lib.lib.as_ref())
            .and_then(|l| l.path.clone());

        // 合并库 URL（项目配置可以覆盖全局配置）
        let lib_url = project_config
            .and_then(|c| c.lib.lib.as_ref())
            .and_then(|l| l.url.clone())
            .or_else(|| {
                global_config
                    .and_then(|c| c.lib.lib.as_ref())
                    .and_then(|l| l.url.clone())
            });

        // 合并环境变量
        let outdir = cli
            .outdir
            .clone()
            .or_else(|| {
                project_config
                    .and_then(|c| c.build.as_ref())
                    .and_then(|b| b.build.as_ref())
                    .and_then(|b| b.outdir.clone())
            })
            .or_else(|| {
                global_config
                    .and_then(|c| c.env.env.as_ref())
                    .and_then(|e| e.outdir.clone())
            });

        let texmfhome = global_config
            .and_then(|c| c.env.env.as_ref())
            .and_then(|e| e.texmfhome.clone());

        let bibinputs = global_config
            .and_then(|c| c.env.env.as_ref())
            .and_then(|e| e.bibinputs.clone());

        let texinputs = global_config
            .and_then(|c| c.env.env.as_ref())
            .and_then(|e| e.texinputs.clone());

        // 合并项目配置
        let entry = cli.entry.clone().or_else(|| {
            project_config
                .and_then(|c| c.project.as_ref())
                .and_then(|p| p.project.as_ref())
                .and_then(|p| p.entry.clone())
        });

        let from = cli.from.clone().or_else(|| {
            project_config
                .and_then(|c| c.project.as_ref())
                .and_then(|p| p.project.as_ref())
                .and_then(|p| p.from.clone())
        });

        let to = cli.to.clone().or_else(|| {
            project_config
                .and_then(|c| c.project.as_ref())
                .and_then(|p| p.project.as_ref())
                .and_then(|p| p.to.clone())
        });

        let target = cli.target.clone().or_else(|| {
            project_config
                .and_then(|c| c.project.as_ref())
                .and_then(|p| p.project.as_ref())
                .and_then(|p| p.target.clone())
        });

        // 合并构建配置
        let metadata_file = project_config
            .and_then(|c| c.build.as_ref())
            .and_then(|b| b.build.as_ref())
            .and_then(|b| b.metadata_file.clone());

        let verbose = cli
            .verbose
            .or_else(|| {
                project_config
                    .and_then(|c| c.build.as_ref())
                    .and_then(|b| b.build.as_ref())
                    .and_then(|b| b.verbose)
            })
            .unwrap_or(false);

        // 合并图片配置
        let figure_paths = project_config
            .and_then(|c| c.figure.as_ref())
            .and_then(|f| f.figure.as_ref())
            .and_then(|f| f.paths.clone())
            .unwrap_or_default();

        let figure_output = project_config
            .and_then(|c| c.figure.as_ref())
            .and_then(|f| f.figure.as_ref())
            .and_then(|f| f.output.clone());

        // 合并 Pandoc 配置
        let pandoc_config = project_config
            .and_then(|c| c.pandoc.as_ref())
            .and_then(|p| p.pandoc.as_ref());

        let pandoc_options = pandoc_config
            .and_then(|p| p.options.clone())
            .unwrap_or_default();

        let pandoc_css = pandoc_config.and_then(|p| p.css.clone());

        let pandoc_from_format = pandoc_config.and_then(|p| p.from_format.clone());
        let pandoc_to_format = pandoc_config.and_then(|p| p.to_format.clone());
        let pandoc_lua_filters = pandoc_config
            .and_then(|p| p.lua_filters.clone())
            .unwrap_or_default();
        let pandoc_template = pandoc_config.and_then(|p| p.template.clone());
        let pandoc_data_dir = pandoc_config.and_then(|p| p.data_dir.clone());
        let pandoc_resource_path = pandoc_config
            .and_then(|p| p.resource_path.clone())
            .unwrap_or_default();
        let pandoc_syntax_highlighting = pandoc_config.and_then(|p| p.syntax_highlighting.clone());
        let pandoc_crossref_yaml = pandoc_config.and_then(|p| p.crossref_yaml.clone());
        let pandoc_python_path = pandoc_config.and_then(|p| p.python_path.clone());
        let pandoc_standalone = pandoc_config.and_then(|p| p.standalone).unwrap_or(true);
        let pandoc_embed_resources = pandoc_config
            .and_then(|p| p.embed_resources)
            .unwrap_or(true);
        let pandoc_lang = pandoc_config.and_then(|p| p.lang.clone());

        // 合并工具路径
        let mut tool_paths = HashMap::new();
        if let Some(global_tools) = global_config.and_then(|c| c.tools.as_ref()) {
            if let Some(tools) = &global_tools.tools {
                if let Some(p) = &tools.pandoc {
                    tool_paths.insert("pandoc".to_string(), Some(p.clone()));
                }
                if let Some(p) = &tools.latexmk {
                    tool_paths.insert("latexmk".to_string(), Some(p.clone()));
                }
                if let Some(p) = &tools.drawio {
                    tool_paths.insert("drawio".to_string(), Some(p.clone()));
                }
                if let Some(p) = &tools.dot {
                    tool_paths.insert("dot".to_string(), Some(p.clone()));
                }
                if let Some(p) = &tools.inkscape {
                    tool_paths.insert("inkscape".to_string(), Some(p.clone()));
                }
                if let Some(p) = &tools.imagemagick {
                    // imagemagick 的命令名可能是 "magick" 或 "convert"
                    tool_paths.insert("imagemagick".to_string(), Some(p.clone()));
                    // 同时支持 "magick" 作为别名
                    tool_paths.insert("magick".to_string(), Some(p.clone()));
                }
                if let Some(p) = &tools.python3 {
                    tool_paths.insert("python3".to_string(), Some(p.clone()));
                }
                if let Some(p) = &tools.latex_engine {
                    tool_paths.insert("latex_engine".to_string(), Some(p.clone()));
                }
                if let Some(p) = &tools.pandoc_crossref {
                    tool_paths.insert("pandoc-crossref".to_string(), Some(p.clone()));
                }
                if let Some(p) = &tools.kroki {
                    tool_paths.insert("kroki".to_string(), Some(p.clone()));
                }
            }
        }
        // CLI 覆盖工具路径
        for (tool, path) in &cli.tool_paths {
            tool_paths.insert(tool.clone(), path.clone());
        }

        // 模板目录
        let template_dir = global_config.and_then(|c| c.template_dir.clone());

        // 合并路径配置
        let mut paths = PathConfig::new();
        // 项目配置优先
        if let Some(project_paths) = project_config
            .and_then(|c| c.paths.as_ref())
            .and_then(|p| p.paths.as_ref())
        {
            paths.merge_from_config(Some(project_paths));
        }
        // 全局配置作为后备
        if let Some(global_paths) = global_config
            .and_then(|c| c.paths.as_ref())
            .and_then(|p| p.paths.as_ref())
        {
            paths.merge_from_config(Some(global_paths));
        }

        Ok(MergedConfig {
            author,
            lib_path,
            lib_url,
            outdir,
            texmfhome,
            bibinputs,
            texinputs,
            entry,
            from,
            to,
            target,
            metadata_file,
            verbose,
            figure_paths,
            figure_output,
            pandoc_options,
            pandoc_css,
            pandoc_from_format,
            pandoc_to_format,
            pandoc_lua_filters,
            pandoc_template,
            pandoc_data_dir,
            pandoc_resource_path,
            pandoc_syntax_highlighting,
            pandoc_crossref_yaml,
            pandoc_python_path,
            pandoc_standalone,
            pandoc_embed_resources,
            pandoc_lang,
            tool_paths,
            template_dir,
            paths,
        })
    }

    /// 获取合并后的配置
    pub fn get_merged(&self) -> &MergedConfig {
        &self.merged
    }

    /// 设置环境变量
    pub fn setup_env(&self) -> Result<()> {
        let merged = &self.merged;

        fn expand_home_placeholders(input: &str) -> String {
            let mut s = input.to_string();
            let home = std::env::var("HOME")
                .ok()
                .or_else(|| dirs::home_dir().map(|p| p.to_string_lossy().to_string()))
                .unwrap_or_default();
            if !home.is_empty() {
                s = s.replace("$ENV{HOME}", &home);
                s = s.replace("$HOME", &home);
                if s.starts_with('~') {
                    s = s.replacen('~', &home, 1);
                }
            }
            s
        }

        if let Some(outdir) = &merged.outdir {
            env::set_var("OUTDIR", outdir);
        }

        // TEXMFHOME: 优先使用配置；若未配置，则使用默认 omnidoc 路径
        if let Some(texmfhome) = &merged.texmfhome {
            let expanded = expand_home_placeholders(texmfhome);
            env::set_var("TEXMFHOME", expanded);
        } else {
            // 默认值基于 lib_path 或 XDG 数据目录：~/.local/share/omnidoc/texmf
            let default_texmf = self
                .merged
                .lib_path
                .as_ref()
                .map(|p| format!("{}/texmf", p))
                .or_else(|| {
                    dirs::data_local_dir().map(|d| {
                        d.join("omnidoc")
                            .join("texmf")
                            .to_string_lossy()
                            .to_string()
                    })
                })
                .unwrap_or_else(|| {
                    if let Some(h) = dirs::home_dir() {
                        h.join(".local")
                            .join("share")
                            .join("omnidoc")
                            .join("texmf")
                            .to_string_lossy()
                            .to_string()
                    } else {
                        ".local/share/omnidoc/texmf".to_string()
                    }
                });
            env::set_var("TEXMFHOME", default_texmf);
        }

        if let Some(bibinputs) = &merged.bibinputs {
            let expanded = expand_home_placeholders(bibinputs);
            env::set_var("BIBINPUTS", expanded);
        }

        if let Some(texinputs) = &merged.texinputs {
            let expanded = expand_home_placeholders(texinputs);
            env::set_var("TEXINPUTS", expanded);
        }

        Ok(())
    }

    /// 获取工具路径（优先使用配置，否则查找系统 PATH）
    pub fn get_tool_path(&self, tool: &str) -> Option<String> {
        if let Some(path) = self.merged.tool_paths.get(tool) {
            if let Some(p) = path {
                return Some(p.clone());
            }
        }

        // 检查系统 PATH
        which::which(tool)
            .ok()
            .and_then(|p| p.to_str().map(|s| s.to_string()))
    }

    /// 获取全局配置
    pub fn global(&self) -> &GlobalConfig {
        &self.global
    }

    /// 获取项目配置
    pub fn project(&self) -> Option<&ProjectConfig> {
        self.project.as_ref()
    }

    /// 获取路径配置
    pub fn paths(&self) -> &PathConfig {
        &self.merged.paths
    }
}
