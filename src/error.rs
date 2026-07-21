use std::borrow::Cow;
use thiserror::Error;

/// Unified error type for OmniDoc
#[derive(Debug, Error)]
pub enum OmniDocError {
    #[error("{0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Config(String),

    #[error("Configuration file not found: {0}")]
    ConfigNotFound(String),

    #[error("{0}")]
    Project(String),

    #[error("This is not an OmniDoc project: {0}")]
    NotOmniDocProject(String),

    #[error("Unsupported document type: {0}")]
    UnsupportedDocumentType(String),

    #[error("Command execution failed: {0}")]
    CommandExecution(String),

    #[error("Command exited with code {code:?}: {command}")]
    CommandNonZeroExit { code: Option<i32>, command: String },

    #[error("Command exited with code {code:?}: {command}\n{output}")]
    CommandFailed {
        code: Option<i32>,
        command: String,
        output: String,
    },

    #[error("HTTP error {status}: {url}")]
    HttpError { status: u16, url: String },

    #[error("{0}")]
    Git(#[from] git2::Error),

    #[error("{0}")]
    Walkdir(#[from] walkdir::Error),

    #[error("Unsupported language: {0}")]
    UnsupportedLanguage(String),

    #[error("{0}")]
    Other(String),
}

impl OmniDocError {
    /// A short, stable category used by the terminal diagnostic renderer.
    pub fn category(&self) -> &'static str {
        match self {
            Self::Io(_) => "filesystem",
            Self::Config(_) | Self::ConfigNotFound(_) => "configuration",
            Self::Project(_) | Self::NotOmniDocProject(_) => "project",
            Self::UnsupportedDocumentType(_) | Self::UnsupportedLanguage(_) => "input",
            Self::CommandExecution(_)
            | Self::CommandNonZeroExit { .. }
            | Self::CommandFailed { .. } => "command",
            Self::HttpError { .. } => "network",
            Self::Git(_) => "git",
            Self::Walkdir(_) => "filesystem",
            Self::Other(_) => "omnidoc",
        }
    }

    /// The useful error payload without the legacy `thiserror` category prefix.
    pub fn message(&self) -> Cow<'_, str> {
        match self {
            Self::Io(error) => Cow::Owned(error.to_string()),
            Self::Config(message) | Self::Project(message) | Self::Other(message) => {
                Cow::Borrowed(message)
            }
            Self::ConfigNotFound(message) => {
                Cow::Owned(format!("Configuration file not found: {message}"))
            }
            Self::NotOmniDocProject(message) => {
                Cow::Owned(format!("This is not an OmniDoc project: {message}"))
            }
            Self::UnsupportedDocumentType(message) => {
                Cow::Owned(format!("Unsupported document type: {message}"))
            }
            Self::CommandExecution(message) => {
                Cow::Owned(format!("Command execution failed: {message}"))
            }
            Self::UnsupportedLanguage(message) => {
                Cow::Owned(format!("Unsupported language: {message}"))
            }
            Self::CommandNonZeroExit { code, command } => {
                Cow::Owned(format!("Command exited with code {code:?}: {command}"))
            }
            Self::CommandFailed {
                code,
                command,
                output,
            } => Cow::Owned(format!(
                "Command exited with code {code:?}: {command}\n{output}"
            )),
            Self::HttpError { status, url } => {
                Cow::Owned(format!("HTTP request returned status {status}: {url}"))
            }
            Self::Git(error) => Cow::Owned(error.message().to_string()),
            Self::Walkdir(error) => Cow::Owned(error.to_string()),
        }
    }

    /// Actionable guidance for error classes where the next step is unambiguous.
    pub fn help(&self) -> Option<&'static str> {
        match self {
            Self::ConfigNotFound(_) => {
                Some("Check the configuration path or create the missing file.")
            }
            Self::NotOmniDocProject(_) => Some(
                "Run this command inside an OmniDoc project, or pass the project path explicitly.",
            ),
            Self::UnsupportedDocumentType(_) => {
                Some("Run `omnidoc doctypes` to list the supported document types.")
            }
            Self::UnsupportedLanguage(_) => {
                Some("Choose one of the languages supported by this command.")
            }
            _ => None,
        }
    }
}

pub type Result<T> = std::result::Result<T, OmniDocError>;
