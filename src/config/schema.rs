use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 配置结构定义
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConfigSchema {
    #[serde(flatten)]
    pub author: AuthorConfig,
    #[serde(flatten)]
    pub lib: LibConfig,
    #[serde(flatten)]
    pub env: EnvConfig,
    #[serde(flatten)]
    pub project: Option<ProjectConfig>,
    #[serde(flatten)]
    pub build: Option<BuildConfig>,
    #[serde(flatten)]
    pub figure: Option<FigureConfig>,
    #[serde(flatten)]
    pub pandoc: Option<PandocConfig>,
    #[serde(flatten)]
    pub tools: Option<ToolsConfig>,
    #[serde(flatten)]
    pub paths: Option<PathsConfig>,
    pub download: Option<Vec<DownloadConfig>>,
    pub template_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct AuthorConfig {
    #[serde(default, rename = "author")]
    pub author: Option<AuthorSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuthorSection {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct LibConfig {
    #[serde(default, rename = "lib")]
    pub lib: Option<LibSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LibSection {
    pub path: Option<String>,
    /// OmniDoc library repository URL (全局配置)
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct EnvConfig {
    #[serde(default, rename = "env")]
    pub env: Option<EnvSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EnvSection {
    pub outdir: Option<String>,
    pub texmfhome: Option<String>,
    pub bibinputs: Option<String>,
    pub texinputs: Option<String>,
}

/// 项目配置（在 .omnidoc.toml 中）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct ProjectConfig {
    #[serde(rename = "project")]
    pub project: Option<ProjectSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectSection {
    /// 入口文件（如 main.md, main.tex）
    pub entry: Option<String>,
    /// 源码类型（markdown, latex）
    pub from: Option<String>,
    /// 生成文档类型（pdf, html, epub）
    pub to: Option<String>,
    /// 生成文档名称（不含扩展名）
    pub target: Option<String>,
}

/// 构建配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct BuildConfig {
    #[serde(rename = "build")]
    pub build: Option<BuildSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BuildSection {
    pub outdir: Option<String>,
    pub metadata_file: Option<String>,
    pub verbose: Option<bool>,
}

/// 图片配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct FigureConfig {
    #[serde(rename = "figure")]
    pub figure: Option<FigureSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FigureSection {
    /// 图片源文件目录
    pub paths: Option<Vec<String>>,
    /// 图片输出目录
    pub output: Option<String>,
}

/// Pandoc 配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct PandocConfig {
    #[serde(rename = "pandoc")]
    pub pandoc: Option<PandocSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PandocSection {
    /// Pandoc 额外选项列表（会被追加到默认选项之后）
    pub options: Option<Vec<String>>,
    /// CSS 文件路径（用于 HTML 输出）
    pub css: Option<String>,
    /// Markdown 格式选项（如 "markdown+east_asian_line_breaks+footnotes"）
    pub from_format: Option<String>,
    /// 输出格式（如 "html", "latex"）
    pub to_format: Option<String>,
    /// Lua filters 列表（相对于 omnidoc-libs/pandoc/lua/）
    pub lua_filters: Option<Vec<String>>,
    /// 模板文件名（相对于 omnidoc-libs/pandoc/data/templates/）
    pub template: Option<String>,
    /// 数据目录（默认使用 omnidoc-libs/pandoc/data）
    pub data_dir: Option<String>,
    /// 资源路径（用冒号分隔的路径列表）
    pub resource_path: Option<Vec<String>>,
    /// 语法高亮样式（如 "idiomatic", "pygments", "kate"）
    pub syntax_highlighting: Option<String>,
    /// Crossref YAML 文件路径（相对于 omnidoc-libs）
    pub crossref_yaml: Option<String>,
    /// Python 路径（用于 Lua filters）
    pub python_path: Option<String>,
    /// 是否使用 standalone 模式
    pub standalone: Option<bool>,
    /// 是否嵌入资源（HTML 输出）
    pub embed_resources: Option<bool>,
    /// 语言代码（用于设置 crossrefYaml）
    pub lang: Option<String>,
}

/// 工具路径配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct ToolsConfig {
    #[serde(rename = "tools")]
    pub tools: Option<ToolsSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolsSection {
    pub pandoc: Option<String>,
    pub latexmk: Option<String>,
    pub drawio: Option<String>,
    pub dot: Option<String>,
    pub inkscape: Option<String>,
    pub imagemagick: Option<String>,
    pub python3: Option<String>,
    /// LaTeX 引擎（xelatex, pdflatex, lualatex）
    pub latex_engine: Option<String>,
    /// pandoc-crossref 可执行文件路径
    pub pandoc_crossref: Option<String>,
    /// kroki 服务 URL 或本地可执行文件路径（用于 mermaid 生成）
    pub kroki: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadConfig {
    pub url: String,
    pub filename: String,
}

/// 路径配置（在配置文件中）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct PathsConfig {
    #[serde(rename = "paths")]
    pub paths: Option<PathsSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PathsSection {
    /// 构建输出目录
    pub build_dir: Option<String>,
    /// Markdown 入口文件
    pub main_md: Option<String>,
    /// LaTeX 入口文件
    pub main_tex: Option<String>,
    /// 图片目录（旧版兼容）
    pub figure_dir: Option<String>,
    /// 图片输出目录
    pub figures_dir: Option<String>,
    /// DAC 目录（dot, mermaid, json）
    pub dac_dir: Option<String>,
    /// Draw.io 目录
    pub drawio_dir: Option<String>,
    /// 参考文献目录
    pub biblio_dir: Option<String>,
    /// Markdown 源码目录
    pub md_dir: Option<String>,
}

/// 路径配置（运行时使用的默认值）
#[derive(Debug, Clone, Default)]
pub struct PathConfig {
    pub build_dir: String,
    pub main_md: String,
    pub main_tex: String,
    pub figure_dir: String,
    pub figures_dir: String,
    pub dac_dir: String,
    pub drawio_dir: String,
    pub biblio_dir: String,
    pub md_dir: String,
}

impl PathConfig {
    pub fn new() -> Self {
        Self {
            build_dir: "build".to_string(),
            main_md: "main.md".to_string(),
            main_tex: "main.tex".to_string(),
            figure_dir: "figure".to_string(),
            figures_dir: "figures".to_string(),
            dac_dir: "dac".to_string(),
            drawio_dir: "drawio".to_string(),
            biblio_dir: "biblio".to_string(),
            md_dir: "md".to_string(),
        }
    }

    /// 从配置合并路径值
    pub fn merge_from_config(&mut self, paths_config: Option<&PathsSection>) {
        if let Some(paths) = paths_config {
            if let Some(v) = &paths.build_dir {
                self.build_dir = v.clone();
            }
            if let Some(v) = &paths.main_md {
                self.main_md = v.clone();
            }
            if let Some(v) = &paths.main_tex {
                self.main_tex = v.clone();
            }
            if let Some(v) = &paths.figure_dir {
                self.figure_dir = v.clone();
            }
            if let Some(v) = &paths.figures_dir {
                self.figures_dir = v.clone();
            }
            if let Some(v) = &paths.dac_dir {
                self.dac_dir = v.clone();
            }
            if let Some(v) = &paths.drawio_dir {
                self.drawio_dir = v.clone();
            }
            if let Some(v) = &paths.biblio_dir {
                self.biblio_dir = v.clone();
            }
            if let Some(v) = &paths.md_dir {
                self.md_dir = v.clone();
            }
        }
    }
}
