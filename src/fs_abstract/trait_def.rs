use crate::error::Result;
use std::path::Path;

/// Trait for file system operations
///
/// This abstraction allows for dependency injection and testing
pub trait FileSystem: Send + Sync {
    /// Create a directory
    fn create_dir(&self, path: &Path) -> Result<()>;

    /// Create a directory and all parent directories
    fn create_dir_all(&self, path: &Path) -> Result<()>;

    /// Remove a file
    fn remove_file(&self, path: &Path) -> Result<()>;

    /// Remove a directory and all its contents
    fn remove_dir_all(&self, path: &Path) -> Result<()>;

    /// Copy a file
    fn copy(&self, from: &Path, to: &Path) -> Result<()>;

    /// Rename/move a file or directory
    fn rename(&self, from: &Path, to: &Path) -> Result<()>;

    /// Read entire file contents as string
    fn read_to_string(&self, path: &Path) -> Result<String>;

    /// Write bytes to a file
    fn write(&self, path: &Path, contents: &[u8]) -> Result<()>;

    /// Check if path exists
    fn exists(&self, path: &Path) -> bool;

    /// Check if path is a file
    fn is_file(&self, path: &Path) -> bool;

    /// Check if path is a directory
    fn is_dir(&self, path: &Path) -> bool;
}
