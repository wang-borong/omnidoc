use crate::diagnostics::summarize_command_output;
use crate::error::{OmniDocError, Result};
use crate::executor::trait_def::{CommandExecutor, CommandOutput};
use std::io::{self, Write};
use std::process::Command;

/// System command executor using std::process::Command
pub struct SystemCommandExecutor;

impl SystemCommandExecutor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SystemCommandExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandExecutor for SystemCommandExecutor {
    fn execute(&self, cmd: &str, args: &[&str]) -> Result<CommandOutput> {
        let output = Command::new(cmd).args(args).output().map_err(|e| {
            OmniDocError::CommandExecution(format!("Failed to execute '{}': {}", cmd, e))
        })?;

        io::stdout()
            .write_all(&output.stdout)
            .map_err(OmniDocError::Io)?;
        io::stderr()
            .write_all(&output.stderr)
            .map_err(OmniDocError::Io)?;

        if !output.status.success() {
            let command = format!("{} {}", cmd, args.join(" "));
            let diagnostic = summarize_command_output(&output.stdout, &output.stderr)
                .unwrap_or_else(|| "No command output was captured.".to_string());
            return Err(OmniDocError::CommandFailed {
                code: output.status.code(),
                command,
                output: diagnostic,
            });
        }

        Ok(CommandOutput {
            stdout: output.stdout,
            stderr: output.stderr,
            status: output.status.code().unwrap_or(-1),
        })
    }

    fn spawn(&self, cmd: &str, args: &[&str]) -> Result<()> {
        Command::new(cmd).args(args).spawn().map_err(|e| {
            OmniDocError::CommandExecution(format!("Failed to spawn '{}': {}", cmd, e))
        })?;
        Ok(())
    }
}
