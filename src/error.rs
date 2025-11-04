use thiserror::Error;

/// Unified error type for OmniDoc
#[derive(Debug, Error)]
pub enum OmniDocError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Configuration file not found: {0}")]
    ConfigNotFound(String),

    #[error("Project error: {0}")]
    Project(String),

    #[error("This is not an OmniDoc project: {0}")]
    NotOmniDocProject(String),

    #[error("Unsupported document type: {0}")]
    UnsupportedDocumentType(String),

    #[error("Command execution failed: {0}")]
    CommandExecution(String),

    #[error("Command exited with code {code:?}: {command}")]
    CommandNonZeroExit { code: Option<i32>, command: String },

    #[error("HTTP error {status}: {url}")]
    HttpError { status: u16, url: String },

    #[error("Git error: {0}")]
    Git(#[from] git2::Error),

    #[error("Unsupported language: {0}")]
    UnsupportedLanguage(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, OmniDocError>;
