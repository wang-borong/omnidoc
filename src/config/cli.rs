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
    pub verbose: Option<bool>,
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

    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = Some(verbose);
        self
    }

    pub fn with_tool_path(mut self, tool: String, path: Option<String>) -> Self {
        self.tool_paths.insert(tool, path);
        self
    }
}
