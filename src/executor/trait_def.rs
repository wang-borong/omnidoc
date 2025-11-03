use crate::error::Result;

/// Output from command execution
#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub status: i32,
}

/// Trait for executing system commands
///
/// This abstraction allows for dependency injection and testing
pub trait CommandExecutor: Send + Sync {
    /// Execute a command and wait for it to complete
    ///
    /// Returns the output if successful, or an error if the command fails
    fn execute(&self, cmd: &str, args: &[&str]) -> Result<CommandOutput>;

    /// Spawn a command and return immediately (non-blocking)
    ///
    /// Does not wait for the command to complete
    fn spawn(&self, cmd: &str, args: &[&str]) -> Result<()>;
}
