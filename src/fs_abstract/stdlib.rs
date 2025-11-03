use crate::error::{OmniDocError, Result};
use crate::fs_abstract::trait_def::FileSystem;
use std::fs;
use std::path::Path;

/// Standard library file system implementation
pub struct StdFileSystem;

impl StdFileSystem {
    pub fn new() -> Self {
        Self
    }
}

impl Default for StdFileSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl FileSystem for StdFileSystem {
    fn create_dir(&self, path: &Path) -> Result<()> {
        fs::create_dir(path).map_err(|e| OmniDocError::Io(e))?;
        Ok(())
    }

    fn create_dir_all(&self, path: &Path) -> Result<()> {
        fs::create_dir_all(path).map_err(|e| OmniDocError::Io(e))?;
        Ok(())
    }

    fn remove_file(&self, path: &Path) -> Result<()> {
        fs::remove_file(path).map_err(|e| OmniDocError::Io(e))?;
        Ok(())
    }

    fn remove_dir_all(&self, path: &Path) -> Result<()> {
        fs::remove_dir_all(path).map_err(|e| OmniDocError::Io(e))?;
        Ok(())
    }

    fn copy(&self, from: &Path, to: &Path) -> Result<()> {
        fs::copy(from, to).map_err(|e| OmniDocError::Io(e))?;
        Ok(())
    }

    fn rename(&self, from: &Path, to: &Path) -> Result<()> {
        fs::rename(from, to).map_err(|e| OmniDocError::Io(e))?;
        Ok(())
    }

    fn read_to_string(&self, path: &Path) -> Result<String> {
        fs::read_to_string(path).map_err(|e| OmniDocError::Io(e))
    }

    fn write(&self, path: &Path, contents: &[u8]) -> Result<()> {
        fs::write(path, contents).map_err(|e| OmniDocError::Io(e))
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }

    fn is_dir(&self, path: &Path) -> bool {
        path.is_dir()
    }
}
