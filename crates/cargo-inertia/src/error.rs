//! Typed errors returned by CLI commands.

use std::{io, path::PathBuf};

use thiserror::Error;

/// Errors produced by `cargo inertia` commands.
#[derive(Debug, Error)]
pub enum CliError {
    /// A frontend destination already exists.
    #[error("frontend directory already exists: {0}")]
    FrontendExists(PathBuf),
    /// An I/O operation failed.
    #[error(transparent)]
    Io(#[from] io::Error),
    /// A legacy command implementation reported an error string.
    #[error("{0}")]
    Message(String),
}

impl From<String> for CliError {
    fn from(value: String) -> Self {
        Self::Message(value)
    }
}
