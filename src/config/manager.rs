use crate::config::cli::CliOverrides;
use crate::config::global::GlobalConfig;
use crate::config::project::ProjectConfig;
use crate::config::schema::*;
use crate::error::Result;
use crate::utils::directories::data_local_dir;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
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
#[derive(Debug, Clone, Default, Serialize)]
pub struct MergedConfig {
    pub author: Option<String>,
    pub lib_path: Option<String>,
    pub outdir: Option<String>,
    pub texmfhome: Option<String>,
    pub bibinputs: Option<String>,
    pub texinputs: Option<String>,
    pub entry: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub outputs: Vec<String>,
    pub target: Option<String>,
    pub metadata_file: Option<String>,
    pub verbose: bool,
    pub latex_backend: String,
    pub max_latex_passes: usize,
    pub figure_paths: Vec<String>,
    pub figure_output: Option<String>,
    pub theme_name: Option<String>,
    pub theme_version: Option<String>,
    pub theme_compatibility: Option<String>,
    pub pandoc_toc: bool,
    pub pandoc_options: Vec<String>,
    pub pandoc_format_options: BTreeMap<String, Vec<String>>,
    pub pandoc_css: Option<String>,
    pub pandoc_reference_doc: Option<String>,
    pub pandoc_pptx_reference_doc: Option<String>,
    pub pandoc_epub_css: Option<String>,
    pub pandoc_from_format: Option<String>,
    pub pandoc_to_format: Option<String>,
    pub pandoc_lua_filters: Vec<String>,
    pub pandoc_template: Option<String>,
    pub pandoc_html_template: Option<String>,
    pub pandoc_latex_template: Option<String>,
    pub pandoc_epub_template: Option<String>,
    pub pandoc_data_dir: Option<String>,
    pub pandoc_resource_path: Vec<String>,
    pub pandoc_syntax_highlighting: Option<String>,
    pub pandoc_crossref_yaml: Option<String>,
    pub pandoc_python_path: Option<String>,
    pub pandoc_standalone: bool,
    pub pandoc_embed_resources: bool,
    pub pandoc_lang: Option<String>,
    pub tectonic_bundle: Option<String>,
    pub tectonic_only_cached: bool,
    pub tectonic_shell_escape: bool,
    pub tectonic_search_paths: Vec<String>,
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

        let outputs = if !cli.outputs.is_empty() {
            cli.outputs.clone()
        } else {
            project_config
                .and_then(|c| c.build.as_ref())
                .and_then(|b| b.build.as_ref())
                .and_then(|b| b.outputs.clone())
                .unwrap_or_default()
        };

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

        let latex_backend = cli
            .latex_backend
            .clone()
            .or_else(|| {
                project_config
                    .and_then(|c| c.build.as_ref())
                    .and_then(|b| b.build.as_ref())
                    .and_then(|b| b.latex_backend.clone())
            })
            .unwrap_or_else(|| "latexmk".to_string());

        let max_latex_passes = cli
            .max_latex_passes
            .or_else(|| {
                project_config
                    .and_then(|c| c.build.as_ref())
                    .and_then(|b| b.build.as_ref())
                    .and_then(|b| b.max_latex_passes)
            })
            .unwrap_or(5);

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

        let project_theme = project_config
            .and_then(|config| config.theme.as_ref())
            .and_then(|theme| theme.theme.as_ref());
        let global_theme = global_config
            .and_then(|config| config.theme.as_ref())
            .and_then(|theme| theme.theme.as_ref());
        let selected_theme = project_theme.or(global_theme);
        let theme_name = selected_theme.and_then(|theme| theme.name.clone());
        let theme_version = selected_theme.and_then(|theme| theme.version.clone());
        let theme_compatibility = selected_theme.and_then(|theme| theme.compatibility.clone());

        // 合并 Pandoc 配置
        let pandoc_config = project_config
            .and_then(|c| c.pandoc.as_ref())
            .and_then(|p| p.pandoc.as_ref());

        let pandoc_toc = pandoc_config.and_then(|p| p.toc).unwrap_or(false);
        let pandoc_options = pandoc_config
            .and_then(|p| p.options.clone())
            .unwrap_or_default();
        let pandoc_format_options = pandoc_config
            .and_then(|p| p.format_options.clone())
            .unwrap_or_default()
            .into_iter()
            .map(|(format, options)| (format.trim().to_ascii_lowercase(), options))
            .collect();

        let pandoc_css = pandoc_config.and_then(|p| p.css.clone());
        let pandoc_reference_doc = pandoc_config.and_then(|p| p.reference_doc.clone());
        let pandoc_pptx_reference_doc = pandoc_config.and_then(|p| p.pptx_reference_doc.clone());
        let pandoc_epub_css = pandoc_config.and_then(|p| p.epub_css.clone());

        let pandoc_from_format = pandoc_config.and_then(|p| p.from_format.clone());
        let pandoc_to_format = pandoc_config.and_then(|p| p.to_format.clone());
        let pandoc_lua_filters = pandoc_config
            .and_then(|p| p.lua_filters.clone())
            .unwrap_or_default();
        let pandoc_template = pandoc_config.and_then(|p| p.template.clone());
        let pandoc_html_template = pandoc_config.and_then(|p| p.html_template.clone());
        let pandoc_latex_template = pandoc_config.and_then(|p| p.latex_template.clone());
        let pandoc_epub_template = pandoc_config.and_then(|p| p.epub_template.clone());
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

        let project_tectonic = project_config
            .and_then(|config| config.tectonic.as_ref())
            .and_then(|config| config.tectonic.as_ref());
        let global_tectonic = global_config
            .and_then(|config| config.tectonic.as_ref())
            .and_then(|config| config.tectonic.as_ref());
        let tectonic_bundle = project_tectonic
            .and_then(|config| config.bundle.clone())
            .or_else(|| global_tectonic.and_then(|config| config.bundle.clone()));
        let tectonic_only_cached = project_tectonic
            .and_then(|config| config.only_cached)
            .or_else(|| global_tectonic.and_then(|config| config.only_cached))
            .unwrap_or(false);
        let tectonic_shell_escape = project_tectonic
            .and_then(|config| config.shell_escape)
            .or_else(|| global_tectonic.and_then(|config| config.shell_escape))
            .unwrap_or(false);
        let tectonic_search_paths = project_tectonic
            .and_then(|config| config.search_paths.clone())
            .or_else(|| global_tectonic.and_then(|config| config.search_paths.clone()))
            .unwrap_or_default();

        // 合并工具路径
        let mut tool_paths = HashMap::new();
        merge_tool_paths(&mut tool_paths, global_config);
        merge_tool_paths(&mut tool_paths, project_config);
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
            outdir,
            texmfhome,
            bibinputs,
            texinputs,
            entry,
            from,
            to,
            outputs,
            target,
            metadata_file,
            verbose,
            latex_backend,
            max_latex_passes,
            figure_paths,
            figure_output,
            theme_name,
            theme_version,
            theme_compatibility,
            pandoc_toc,
            pandoc_options,
            pandoc_format_options,
            pandoc_css,
            pandoc_reference_doc,
            pandoc_pptx_reference_doc,
            pandoc_epub_css,
            pandoc_from_format,
            pandoc_to_format,
            pandoc_lua_filters,
            pandoc_template,
            pandoc_html_template,
            pandoc_latex_template,
            pandoc_epub_template,
            pandoc_data_dir,
            pandoc_resource_path,
            pandoc_syntax_highlighting,
            pandoc_crossref_yaml,
            pandoc_python_path,
            pandoc_standalone,
            pandoc_embed_resources,
            pandoc_lang,
            tectonic_bundle,
            tectonic_only_cached,
            tectonic_shell_escape,
            tectonic_search_paths,
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
                    data_local_dir().map(|d| {
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
        if let Some(Some(path)) = self.merged.tool_paths.get(tool) {
            return Some(path.clone());
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

fn merge_tool_paths(target: &mut HashMap<String, Option<String>>, config: Option<&ConfigSchema>) {
    let Some(tools) = config
        .and_then(|config| config.tools.as_ref())
        .and_then(|config| config.tools.as_ref())
    else {
        return;
    };
    for (key, value) in [
        ("pandoc", tools.pandoc.as_ref()),
        ("latexmk", tools.latexmk.as_ref()),
        ("drawio", tools.drawio.as_ref()),
        ("dot", tools.dot.as_ref()),
        ("inkscape", tools.inkscape.as_ref()),
        ("python3", tools.python3.as_ref()),
        ("kicad-cli", tools.kicad_cli.as_ref()),
        ("ngspice", tools.ngspice.as_ref()),
        ("latex_engine", tools.latex_engine.as_ref()),
        ("tectonic", tools.tectonic.as_ref()),
        ("pandoc-crossref", tools.pandoc_crossref.as_ref()),
        ("epubcheck", tools.epubcheck.as_ref()),
        ("kroki", tools.kroki.as_ref()),
    ] {
        if let Some(value) = value {
            target.insert(key.to_string(), Some(value.clone()));
        }
    }
    if let Some(value) = tools.imagemagick.as_ref() {
        target.insert("imagemagick".to_string(), Some(value.clone()));
        target.insert("magick".to_string(), Some(value.clone()));
    }
}

#[cfg(test)]
mod tests {
    use super::merge_tool_paths;
    use crate::config::schema::ConfigSchema;
    use std::collections::HashMap;

    #[test]
    fn project_tools_override_global_tools() {
        let global: ConfigSchema =
            toml::from_str("[tools]\npandoc = 'global-pandoc'\ntectonic = 'global-tectonic'\n")
                .expect("global config");
        let project: ConfigSchema =
            toml::from_str("[tools]\ntectonic = 'project-tectonic'\n").expect("project config");
        let mut paths = HashMap::new();

        merge_tool_paths(&mut paths, Some(&global));
        merge_tool_paths(&mut paths, Some(&project));

        assert_eq!(
            paths.get("pandoc").and_then(|value| value.as_deref()),
            Some("global-pandoc")
        );
        assert_eq!(
            paths.get("tectonic").and_then(|value| value.as_deref()),
            Some("project-tectonic")
        );
    }
}
