use crate::error::{OmniDocError, Result};
use std::io::{self, Write};
use std::process::Command;

pub struct CommandOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub status: i32,
}

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
