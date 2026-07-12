//! Typed errors returned by CLI commands.

use std::{io, path::PathBuf};

use thiserror::Error;

/// Errors produced by `cargo inertia` commands.
#[derive(Debug, Error)]
pub enum CliError {
    /// A frontend destination already exists.
    #[error("frontend directory already exists: {0}")]
    FrontendExists(PathBuf),
    /// The user cancelled initialization before files were generated.
    #[error("initialization cancelled")]
    Cancelled,
    /// Required initialization input is missing or inconsistent.
    #[error("invalid initialization options: {0}")]
    InvalidOptions(String),
    /// A destination cannot be used for atomic scaffolding.
    #[error("invalid frontend destination: {0}")]
    InvalidDestination(PathBuf),
    /// A generated output path is unsafe.
    #[error("unsafe generated output path: {0}")]
    UnsafeOutputPath(PathBuf),
    /// An explicitly catalogued embedded template was not found.
    #[error("embedded template is missing: {0}")]
    MissingEmbeddedTemplate(&'static str),
    /// An embedded template cannot be decoded as UTF-8.
    #[error("embedded template is not UTF-8: {0}")]
    TemplateIsNotUtf8(&'static str),
    /// A source template is invalid.
    #[error("invalid template syntax")]
    TemplateSyntax(#[source] minijinja::Error),
    /// A source template could not be rendered.
    #[error("could not render template `{template}`")]
    Template {
        template: String,
        #[source]
        source: minijinja::Error,
    },
    /// The staged frontend could not be moved into place.
    #[error("could not commit generated frontend")]
    CommitScaffold {
        from: PathBuf,
        to: PathBuf,
        #[source]
        source: io::Error,
    },
    /// An I/O operation failed.
    #[error(transparent)]
    Io(#[from] io::Error),
    /// Generated JSON is invalid.
    #[cfg(feature = "templates")]
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// A legacy command implementation reported an error string.
    #[error("{0}")]
    Message(String),
}

impl From<String> for CliError {
    fn from(value: String) -> Self {
        Self::Message(value)
    }
}
