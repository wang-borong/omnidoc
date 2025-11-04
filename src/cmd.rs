use crate::error::{OmniDocError, Result};
use std::io::{self, Write};
use std::process::Command;

/// Output from command execution
///
/// # Deprecated
/// 此类型已被弃用。新代码应该使用 `BuildExecutor` 或 `executor::CommandOutput`。
#[deprecated(
    note = "Use BuildExecutor or executor::CommandOutput instead. This will be removed in a future version."
)]
pub struct CommandOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub status: i32,
}

/// Execute a command
///
/// # Deprecated
/// 此函数已被弃用。新代码应该使用 `BuildExecutor::execute_system_cmd()` 或 `BuildExecutor::spawn_system_cmd()`。
///
/// 迁移示例：
/// ```rust
/// // 旧代码
/// do_cmd("make", &["-f", "file.mk"], false)?;
///
/// // 新代码（同步执行）
/// let executor = BuildExecutor::new(HashMap::new());
/// executor.execute_system_cmd("make", &["-f", "file.mk"], false)?;
///
/// // 新代码（异步执行）
/// executor.spawn_system_cmd("xdg-open", &["file.pdf"])?;
/// ```
#[deprecated(
    note = "Use BuildExecutor::execute_system_cmd() or BuildExecutor::spawn_system_cmd() instead. This will be removed in a future version."
)]
pub fn do_cmd(cmd: &str, args: &[&str], nw: bool) -> Result<()> {
    if nw {
        Command::new(cmd).args(args).spawn().map_err(|e| {
            OmniDocError::CommandExecution(format!("Failed to spawn '{}': {}", cmd, e))
        })?;
    } else {
        let output = Command::new(cmd).args(args).output().map_err(|e| {
            OmniDocError::CommandExecution(format!("Failed to execute '{}': {}", cmd, e))
        })?;

        io::stdout()
            .write_all(&output.stdout)
            .map_err(|e| OmniDocError::Io(e))?;
        io::stderr()
            .write_all(&output.stderr)
            .map_err(|e| OmniDocError::Io(e))?;

        if !output.status.success() {
            let command = format!("{} {}", cmd, args.join(" "));
            return Err(OmniDocError::CommandNonZeroExit {
                code: output.status.code(),
                command,
            });
        }
    }

    Ok(())
}

/// Execute a command and return output
///
/// # Deprecated
/// 此函数已被弃用。新代码应该使用 `BuildExecutor::execute_with_output()`。
///
/// 迁移示例：
/// ```rust
/// // 旧代码
/// let output = execute_command("cmd", &["arg"])?;
///
/// // 新代码
/// let executor = BuildExecutor::new(HashMap::new());
/// let output = executor.execute_with_output("cmd", &["arg"])?;
/// ```
#[deprecated(
    note = "Use BuildExecutor::execute_with_output() instead. This will be removed in a future version."
)]
pub fn execute_command(cmd: &str, args: &[&str]) -> Result<CommandOutput> {
    let output = Command::new(cmd).args(args).output().map_err(|e| {
        OmniDocError::CommandExecution(format!("Failed to execute '{}': {}", cmd, e))
    })?;

    Ok(CommandOutput {
        stdout: output.stdout,
        stderr: output.stderr,
        status: output.status.code().unwrap_or(-1),
    })
}
