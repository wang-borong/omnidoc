/// CLI 参数覆盖
/// 用于存储命令行参数，这些参数会覆盖配置文件的设置
#[derive(Debug, Clone, Default)]
pub struct CliOverrides {
    pub author: Option<String>,
    pub outdir: Option<String>,
    pub target: Option<String>,
    pub entry: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub outputs: Vec<String>,
    pub verbose: Option<bool>,
    pub latex_backend: Option<String>,
    pub max_latex_passes: Option<usize>,
    pub tool_paths: HashMap<String, Option<String>>,
}

use std::collections::HashMap;

impl CliOverrides {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_author(mut self, author: Option<String>) -> Self {
        self.author = author;
        self
    }

    pub fn with_outdir(mut self, outdir: Option<String>) -> Self {
        self.outdir = outdir;
        self
    }

    pub fn with_target(mut self, target: Option<String>) -> Self {
        self.target = target;
        self
    }

    pub fn with_entry(mut self, entry: Option<String>) -> Self {
        self.entry = entry;
        self
    }

    pub fn with_from(mut self, from: Option<String>) -> Self {
        self.from = from;
        self
    }

    pub fn with_to(mut self, to: Option<String>) -> Self {
        self.to = to;
        self
    }

    pub fn with_outputs(mut self, outputs: Vec<String>) -> Self {
        self.outputs = outputs;
        self
    }

    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = Some(verbose);
        self
    }

    pub fn with_latex_backend(mut self, latex_backend: Option<String>) -> Self {
        self.latex_backend = latex_backend;
        self
    }

    pub fn with_max_latex_passes(mut self, max_latex_passes: Option<usize>) -> Self {
        self.max_latex_passes = max_latex_passes;
        self
    }

    pub fn with_tool_path(mut self, tool: String, path: Option<String>) -> Self {
        self.tool_paths.insert(tool, path);
        self
    }
}
