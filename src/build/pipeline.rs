use crate::error::Result;
use std::path::Path;

/// 构建管道 trait
/// 定义构建流程的通用接口
pub trait BuildPipeline {
    /// 执行构建
    fn build(&self, project_path: &Path, verbose: bool) -> Result<()>;

    /// 检测项目类型
    fn detect_project_type(&self, project_path: &Path) -> Result<ProjectType>;
}

/// 项目类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectType {
    Markdown,
    Latex,
    Unknown,
}

impl ProjectType {
    /// 从文件扩展名判断项目类型
    pub fn from_entry_file(path: &Path) -> Self {
        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            match ext.to_lowercase().as_str() {
                "md" => ProjectType::Markdown,
                "tex" => ProjectType::Latex,
                _ => ProjectType::Unknown,
            }
        } else {
            ProjectType::Unknown
        }
    }
}
