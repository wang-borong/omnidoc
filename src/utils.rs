//! 统一工具模块
//! 提供通用的错误转换、文件操作、路径处理等辅助函数

use crate::error::{OmniDocError, Result};
use std::io;
use std::path::{Path, PathBuf};

/// 错误转换辅助函数
/// 简化错误转换代码
pub mod error {
    use super::*;
    use git2;

    /// 将 IO 错误转换为 OmniDocError::Io
    pub fn io_err<T>(result: std::result::Result<T, io::Error>) -> Result<T> {
        result.map_err(OmniDocError::Io)
    }

    /// 将错误转换为 OmniDocError::Config
    pub fn config_err<T, E: std::fmt::Display>(
        result: std::result::Result<T, E>,
        msg: impl Into<String>,
    ) -> Result<T> {
        result.map_err(|e| OmniDocError::Config(format!("{}: {}", msg.into(), e)))
    }

    /// 将错误转换为 OmniDocError::Project
    pub fn project_err<T, E: std::fmt::Display>(
        result: std::result::Result<T, E>,
        msg: impl Into<String>,
    ) -> Result<T> {
        result.map_err(|e| OmniDocError::Project(format!("{}: {}", msg.into(), e)))
    }

    /// 将 Git 错误转换为 OmniDocError::Git
    ///
    /// git2::Error 已经通过 From trait 自动转换为 OmniDocError::Git
    /// 此函数提供一致的错误转换接口
    pub fn git_err<T>(result: std::result::Result<T, git2::Error>) -> Result<T> {
        result.map_err(OmniDocError::from)
    }
}

/// 文件操作辅助函数
/// 统一使用 Result<OmniDocError> 类型
pub mod fs {
    use super::*;
    use std::fs;

    /// 创建目录（包括所有父目录）
    pub fn create_dir_all(path: impl AsRef<Path>) -> Result<()> {
        error::io_err(fs::create_dir_all(path.as_ref()))
    }

    /// 读取文件内容为字符串
    pub fn read_to_string(path: impl AsRef<Path>) -> Result<String> {
        error::io_err(fs::read_to_string(path.as_ref()))
    }

    /// 写入文件
    pub fn write(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> Result<()> {
        error::io_err(fs::write(path.as_ref(), contents.as_ref()))
    }

    /// 检查路径是否存在
    pub fn exists(path: impl AsRef<Path>) -> bool {
        path.as_ref().exists()
    }

    /// 检查是否为文件
    pub fn is_file(path: impl AsRef<Path>) -> bool {
        path.as_ref().is_file()
    }

    /// 检查是否为目录
    pub fn is_dir(path: impl AsRef<Path>) -> bool {
        path.as_ref().is_dir()
    }

    /// 删除文件
    pub fn remove_file(path: impl AsRef<Path>) -> Result<()> {
        error::io_err(fs::remove_file(path.as_ref()))
    }

    /// 删除目录（包括所有内容）
    pub fn remove_dir_all(path: impl AsRef<Path>) -> Result<()> {
        error::io_err(fs::remove_dir_all(path.as_ref()))
    }

    /// 复制文件
    pub fn copy(from: impl AsRef<Path>, to: impl AsRef<Path>) -> Result<u64> {
        error::io_err(fs::copy(from.as_ref(), to.as_ref()))
    }

    /// 重命名/移动文件或目录
    pub fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> Result<()> {
        error::io_err(fs::rename(from.as_ref(), to.as_ref()))
    }

    /// 读取目录内容
    /// 返回 ReadDir 迭代器，需要手动处理错误
    pub fn read_dir(path: impl AsRef<Path>) -> Result<std::fs::ReadDir> {
        error::io_err(fs::read_dir(path.as_ref()))
    }
}

/// 路径处理辅助函数
pub mod path {
    use super::*;

    /// 获取当前工作目录
    pub fn current_dir() -> Result<PathBuf> {
        error::io_err(std::env::current_dir())
    }

    /// 设置当前工作目录
    pub fn set_current_dir(path: impl AsRef<Path>) -> Result<()> {
        error::io_err(std::env::set_current_dir(path.as_ref()))
    }

    /// 规范化路径（解析相对路径、.. 等）
    pub fn canonicalize(path: impl AsRef<Path>) -> Result<PathBuf> {
        error::io_err(path.as_ref().canonicalize())
    }

    /// 获取路径的父目录
    pub fn parent(path: &Path) -> Option<&Path> {
        path.parent()
    }

    /// 获取路径的文件名
    pub fn file_name(path: &Path) -> Option<&std::ffi::OsStr> {
        path.file_name()
    }

    /// 获取路径的文件名（字符串）
    pub fn file_name_str(path: &Path) -> Option<&str> {
        path.file_name().and_then(|n| n.to_str())
    }

    /// 获取路径的文件干（不带扩展名）
    pub fn file_stem(path: &Path) -> Option<&std::ffi::OsStr> {
        path.file_stem()
    }

    /// 获取路径的文件干（字符串）
    pub fn file_stem_str(path: &Path) -> Option<&str> {
        path.file_stem().and_then(|s| s.to_str())
    }

    /// 获取路径的扩展名
    pub fn extension(path: &Path) -> Option<&std::ffi::OsStr> {
        path.extension()
    }

    /// 获取路径的扩展名（字符串）
    pub fn extension_str(path: &Path) -> Option<&str> {
        path.extension().and_then(|e| e.to_str())
    }

    /// 从字符串创建 PathBuf
    pub fn from_str(path: impl AsRef<str>) -> PathBuf {
        PathBuf::from(path.as_ref())
    }

    /// 从 String 创建 PathBuf
    pub fn from_string(path: String) -> PathBuf {
        PathBuf::from(path)
    }

    /// 从字符串创建 Path 引用
    ///
    /// 注意：此函数返回的是临时引用，生命周期受参数限制。
    /// 对于静态路径字符串，直接使用 `Path::new()`。
    pub fn new_path<'a>(path: &'a str) -> &'a Path {
        Path::new(path)
    }

    /// 路径拼接（带错误处理）
    pub fn join(base: impl AsRef<Path>, path: impl AsRef<Path>) -> PathBuf {
        base.as_ref().join(path.as_ref())
    }

    /// 解析相对路径（基于当前目录）
    pub fn resolve(path: impl AsRef<Path>) -> Result<PathBuf> {
        let path = path.as_ref();
        if path.is_absolute() {
            Ok(path.to_path_buf())
        } else {
            let current = current_dir()?;
            Ok(current.join(path))
        }
    }

    /// 规范化路径（解析相对路径、.. 等）
    pub fn normalize(path: impl AsRef<Path>) -> Result<PathBuf> {
        canonicalize(path)
    }

    /// 确定项目路径（从可选路径字符串或当前目录）
    pub fn determine_project_path(path: Option<String>) -> Result<PathBuf> {
        if let Some(p) = path {
            Ok(PathBuf::from(p))
        } else {
            current_dir()
        }
    }

    /// 验证项目路径（检查路径是否存在）
    pub fn validate_project_path(path: &Path) -> Result<()> {
        use crate::utils::fs;
        if !fs::exists(path) {
            return Err(OmniDocError::Project(format!(
                "Path does not exist: {}",
                path.display()
            )));
        }
        Ok(())
    }

    /// 检查是否为 omnidoc 项目
    pub fn check_omnidoc_project(project_path: &Path) -> Result<()> {
        use crate::doc::is_omnidoc_project;

        let original_dir = current_dir()?;
        set_current_dir(project_path)?;
        let is_project = is_omnidoc_project();
        set_current_dir(&original_dir)?;

        if !is_project {
            return Err(OmniDocError::NotOmniDocProject(format!(
                "The directory '{}' is not an OmniDoc project",
                project_path.display()
            )));
        }

        Ok(())
    }

    /// 确定输出路径（用于单文件或多文件输入）
    pub fn determine_output_path(
        input_path: &Path,
        output: Option<&str>,
        inputs_count: usize,
        default_extension: &str,
    ) -> PathBuf {
        if let Some(out) = output {
            if inputs_count == 1 {
                // Single input: use specified output
                PathBuf::from(out)
            } else {
                // Multiple inputs: check if output is a directory
                let out_path = Path::new(out);
                if out_path.is_dir() {
                    let file_name = file_stem_str(input_path).unwrap_or("output");
                    out_path.join(format!("{}.{}", file_name, default_extension))
                } else {
                    // Not a directory, use default rule
                    let mut out = input_path.to_path_buf();
                    out.set_extension(default_extension);
                    out
                }
            }
        } else {
            // No output specified, use default rule
            let mut out = input_path.to_path_buf();
            out.set_extension(default_extension);
            out
        }
    }

    /// 确保目录存在（如果不存在则创建）
    pub fn ensure_dir(path: impl AsRef<Path>) -> Result<()> {
        use crate::utils::fs;
        if !fs::exists(&path) {
            fs::create_dir_all(&path)?;
        }
        Ok(())
    }
}

/// 环境变量辅助函数
pub mod env {
    use super::*;

    /// 获取环境变量
    pub fn var(key: impl AsRef<str>) -> Result<String> {
        std::env::var(key.as_ref()).map_err(|_| {
            OmniDocError::Config(format!("Environment variable '{}' not found", key.as_ref()))
        })
    }

    /// 设置环境变量
    pub fn set_var(key: impl AsRef<str>, val: impl AsRef<str>) {
        std::env::set_var(key.as_ref(), val.as_ref());
    }
}
