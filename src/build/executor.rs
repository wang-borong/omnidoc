use crate::constants::pandoc;
use crate::diagnostics::summarize_command_output;
use crate::error::{OmniDocError, Result};
use std::io::Write;
use std::path::{Path, PathBuf};
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

                    return Err(OmniDocError::Other(format!(
                        "Configured LaTeX engine '{}' not found. Please install it or update the latex_engine setting.",
                        path
                    )));
                }
                // 默认使用 xelatex
                pandoc::DEFAULT_ENGINE_LATEX
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
        self.execute_in_dir(cmd, args, verbose, None)
    }

    pub fn execute_in_dir(
        &self,
        cmd: &str,
        args: &[&str],
        verbose: bool,
        working_dir: Option<&Path>,
    ) -> Result<()> {
        let tool_path = self.check_tool(cmd)?;

        let mut command = Command::new(&tool_path);
        command.args(args);
        if let Some(working_dir) = working_dir {
            command.current_dir(working_dir);
        }

        if verbose {
            println!("Executing: {} {}", tool_path, args.join(" "));
        }

        let output = command.output().map_err(|e| {
            OmniDocError::CommandExecution(format!("Failed to execute '{}': {}", cmd, e))
        })?;

        if !output.status.success() {
            let command = format!("{} {}", tool_path, args.join(" "));
            let diagnostic = summarize_command_output(&output.stdout, &output.stderr)
                .unwrap_or_else(|| "No command output was captured.".to_string());

            return Err(OmniDocError::CommandFailed {
                code: output.status.code(),
                command,
                output: diagnostic,
            });
        }

        if verbose {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if !stdout.is_empty() {
                print!("{}", stdout);
            }
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.is_empty() {
                eprint!("{}", stderr);
            }
        }

        Ok(())
    }

    /// 执行命令并返回输出
    pub fn execute_with_output(&self, cmd: &str, args: &[&str]) -> Result<String> {
        let tool_path = self.check_tool(cmd)?;

        let output = Command::new(&tool_path).args(args).output().map_err(|e| {
            OmniDocError::CommandExecution(format!("Failed to execute '{}': {}", cmd, e))
        })?;

        if !output.status.success() {
            let command = format!("{} {}", tool_path, args.join(" "));
            let diagnostic = summarize_command_output(&output.stdout, &output.stderr)
                .unwrap_or_else(|| "No command output was captured.".to_string());

            return Err(OmniDocError::CommandFailed {
                code: output.status.code(),
                command,
                output: diagnostic,
            });
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// 异步执行命令（不等待完成）
    /// 用于启动后台进程或打开文件等场景
    pub fn spawn(&self, cmd: &str, args: &[&str]) -> Result<()> {
        let tool_path = self.check_tool(cmd)?;

        Command::new(&tool_path).args(args).spawn().map_err(|e| {
            OmniDocError::CommandExecution(format!("Failed to spawn '{}': {}", cmd, e))
        })?;

        Ok(())
    }

    /// 执行命令（不检查工具路径，直接使用命令名）
    /// 用于执行系统命令（如 make, xdg-open）等不需要检查工具的场景
    pub fn execute_system_cmd(&self, cmd: &str, args: &[&str], verbose: bool) -> Result<()> {
        let mut command = Command::new(cmd);
        command.args(args);

        if verbose {
            println!("Executing: {} {}", cmd, args.join(" "));
        }

        let output = command.output().map_err(|e| {
            OmniDocError::CommandExecution(format!("Failed to execute '{}': {}", cmd, e))
        })?;

        // 输出 stdout 和 stderr
        std::io::stdout()
            .write_all(&output.stdout)
            .map_err(OmniDocError::Io)?;
        std::io::stderr()
            .write_all(&output.stderr)
            .map_err(OmniDocError::Io)?;

        if !output.status.success() {
            let command_str = format!("{} {}", cmd, args.join(" "));
            let diagnostic = summarize_command_output(&output.stdout, &output.stderr)
                .unwrap_or_else(|| "No command output was captured.".to_string());
            return Err(OmniDocError::CommandFailed {
                code: output.status.code(),
                command: command_str,
                output: diagnostic,
            });
        }

        Ok(())
    }

    /// 异步执行系统命令（不检查工具路径，直接使用命令名）
    /// 用于启动后台进程或打开文件等场景
    pub fn spawn_system_cmd(&self, cmd: &str, args: &[&str]) -> Result<()> {
        Command::new(cmd).args(args).spawn().map_err(|e| {
            OmniDocError::CommandExecution(format!("Failed to spawn '{}': {}", cmd, e))
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::BuildExecutor;
    use std::collections::HashMap;

    #[test]
    fn configured_missing_latex_engine_does_not_fallback() {
        let mut tool_paths = HashMap::new();
        tool_paths.insert(
            "latex_engine".to_string(),
            Some("__omnidoc_missing_latex_engine__".to_string()),
        );
        let executor = BuildExecutor::new(tool_paths);

        let err = executor
            .check_tool("latex_engine")
            .expect_err("missing configured engine should fail");

        assert!(err.to_string().contains("__omnidoc_missing_latex_engine__"));
    }

    #[cfg(unix)]
    #[test]
    fn executes_commands_in_the_requested_working_directory() {
        let directory = tempfile::tempdir().expect("working directory");
        let executor = BuildExecutor::new(HashMap::new());
        let expected = directory.path().to_string_lossy().to_string();

        executor
            .execute_in_dir(
                "sh",
                &["-c", "test \"$PWD\" = \"$1\"", "sh", &expected],
                false,
                Some(directory.path()),
            )
            .expect("command should see requested working directory");
    }
}
