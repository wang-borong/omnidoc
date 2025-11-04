use crate::error::{OmniDocError, Result};
use std::path::PathBuf;
use std::process::Command;

/// 构建执行器
/// 负责工具检查和命令执行
pub struct BuildExecutor {
    tool_paths: std::collections::HashMap<String, Option<String>>,
}

impl BuildExecutor {
    pub fn new(tool_paths: std::collections::HashMap<String, Option<String>>) -> Self {
        Self { tool_paths }
    }

    /// 检查工具是否存在
    pub fn check_tool(&self, tool: &str) -> Result<String> {
        // 工具名称映射（用于兼容性）
        let tool_name = match tool {
            "latex_engine" => {
                // latex_engine 配置可能包含 "xelatex", "pdflatex" 等
                // 首先检查配置的值
                if let Some(Some(path)) = self.tool_paths.get("latex_engine") {
                    // 如果配置的是完整路径，直接使用
                    if PathBuf::from(path).exists() {
                        return Ok(path.clone());
                    }
                    // 如果配置的是工具名（如 "xelatex"），使用该工具名查找
                    if let Ok(path) = which::which(path) {
                        return Ok(path.to_string_lossy().to_string());
                    }
                }
                // 默认使用 xelatex
                "xelatex"
            }
            _ => tool,
        };

        // 首先检查配置的路径
        if let Some(Some(path)) = self.tool_paths.get(tool_name) {
            if PathBuf::from(path).exists() {
                return Ok(path.clone());
            }
        }

        // 检查系统 PATH
        if let Ok(path) = which::which(tool_name) {
            return Ok(path.to_string_lossy().to_string());
        }

        Err(OmniDocError::Other(format!(
            "Tool '{}' not found. Please install it or configure the path in config file.",
            tool_name
        )))
    }

    /// 执行命令
    pub fn execute(&self, cmd: &str, args: &[&str], verbose: bool) -> Result<()> {
        let tool_path = self.check_tool(cmd)?;
        
        let mut command = Command::new(&tool_path);
        command.args(args);

        if verbose {
            println!("Executing: {} {}", tool_path, args.join(" "));
        }

        let output = command.output()
            .map_err(|e| OmniDocError::CommandExecution(format!("Failed to execute '{}': {}", cmd, e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(OmniDocError::CommandNonZeroExit {
                code: output.status.code(),
                command: format!("{} {}", tool_path, args.join(" ")),
            });
        }

        if verbose {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if !stdout.is_empty() {
                print!("{}", stdout);
            }
        }

        Ok(())
    }

    /// 执行命令并返回输出
    pub fn execute_with_output(&self, cmd: &str, args: &[&str]) -> Result<String> {
        let tool_path = self.check_tool(cmd)?;
        
        let output = Command::new(&tool_path)
            .args(args)
            .output()
            .map_err(|e| OmniDocError::CommandExecution(format!("Failed to execute '{}': {}", cmd, e)))?;

        if !output.status.success() {
            return Err(OmniDocError::CommandNonZeroExit {
                code: output.status.code(),
                command: format!("{} {}", tool_path, args.join(" ")),
            });
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

