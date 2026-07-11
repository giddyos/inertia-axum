use axum::http::StatusCode;
use bytes::Bytes;
use std::fmt;

/// A failure while validating or starting an SSR backend.
#[derive(Debug)]
pub enum SsrStartError {
    /// The endpoint is not a valid URI.
    InvalidEndpoint {
        endpoint: String,
        source: axum::http::uri::InvalidUri,
    },
    /// Only absolute HTTP endpoints are currently supported.
    UnsupportedEndpoint(String),
    /// Render timeouts must be greater than zero.
    InvalidTimeout,
    /// Render concurrency must be greater than zero.
    InvalidConcurrency,
    /// Response limits must be greater than zero.
    InvalidResponseLimit,
    /// A configured external bundle could not be validated.
    BundleUnavailable {
        /// Resolved bundle path.
        bundle: std::path::PathBuf,
        /// Filesystem validation failure.
        source: std::io::Error,
    },
    /// The backend did not become healthy before startup timed out.
    HealthTimeout { source: Option<SsrFailure> },
    /// Node version output was malformed.
    InvalidNodeVersion(String),
    /// The Node executable could not be invoked.
    NodeUnavailable {
        runtime: std::path::PathBuf,
        source: std::io::Error,
    },
    /// `node --version` returned a failure status.
    NodeVersionCommandFailed(std::process::ExitStatus),
    /// The installed Node major version is too old.
    UnsupportedNode { found: String, required: u64 },
    /// The configured bundle path is not a file.
    BundleIsNotFile(std::path::PathBuf),
    /// The Node child process could not be spawned.
    NodeSpawn {
        runtime: std::path::PathBuf,
        bundle: std::path::PathBuf,
        source: std::io::Error,
    },
}

impl fmt::Display for SsrStartError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEndpoint { endpoint, .. } => {
                write!(formatter, "invalid SSR endpoint: {endpoint}")
            }
            Self::UnsupportedEndpoint(endpoint) => write!(
                formatter,
                "unsupported SSR endpoint (expected absolute http:// URI): {endpoint}"
            ),
            Self::InvalidTimeout => {
                formatter.write_str("SSR render timeout must be greater than zero")
            }
            Self::InvalidConcurrency => {
                formatter.write_str("SSR maximum concurrency must be greater than zero")
            }
            Self::InvalidResponseLimit => {
                formatter.write_str("SSR maximum response size must be greater than zero")
            }
            Self::BundleUnavailable { bundle, source } => {
                write!(
                    formatter,
                    "SSR bundle {} is unavailable: {source}",
                    bundle.display()
                )
            }
            Self::HealthTimeout { .. } => {
                formatter.write_str("SSR backend did not become healthy before the startup timeout")
            }
            Self::InvalidNodeVersion(value) => write!(formatter, "invalid Node version: {value}"),
            Self::NodeUnavailable { runtime, source } => write!(
                formatter,
                "Node executable {} is unavailable: {source}",
                runtime.display()
            ),
            Self::NodeVersionCommandFailed(status) => {
                write!(formatter, "Node version command failed with {status}")
            }
            Self::UnsupportedNode { found, required } => write!(
                formatter,
                "Node {found} is unsupported; Node {required} or newer is required"
            ),
            Self::BundleIsNotFile(bundle) => {
                write!(formatter, "SSR bundle {} is not a file", bundle.display())
            }
            Self::NodeSpawn {
                runtime,
                bundle,
                source,
            } => write!(
                formatter,
                "could not spawn {} with bundle {}: {source}",
                runtime.display(),
                bundle.display()
            ),
        }
    }
}

impl std::error::Error for SsrStartError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidEndpoint { source, .. } => Some(source),
            Self::BundleUnavailable { source, .. } => Some(source),
            Self::NodeUnavailable { source, .. } | Self::NodeSpawn { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Application startup failure from asset or SSR configuration.
#[derive(Debug)]
pub enum StartError {
    /// Asset configuration failed.
    Config(crate::ConfigError),
    /// SSR startup failed.
    Ssr(SsrStartError),
}

impl fmt::Display for StartError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(error) => error.fmt(formatter),
            Self::Ssr(error) => error.fmt(formatter),
        }
    }
}
impl std::error::Error for StartError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Config(error) => Some(error),
            Self::Ssr(error) => Some(error),
        }
    }
}
impl From<crate::ConfigError> for StartError {
    fn from(error: crate::ConfigError) -> Self {
        Self::Config(error)
    }
}
impl From<SsrStartError> for StartError {
    fn from(error: SsrStartError) -> Self {
        Self::Ssr(error)
    }
}

/// A classified SSR render/runtime failure.
#[derive(Debug)]
pub enum SsrFailure {
    Unavailable,
    Overloaded,
    Timeout,
    Transport(String),
    InvalidResponse(String),
    Render { status: StatusCode, body: String },
    ResponseTooLarge,
    ProcessExited,
}

impl SsrFailure {
    pub(crate) fn request(error: axum::http::Error) -> Self {
        Self::Transport(error.to_string())
    }
    pub(crate) fn service(error: tower::BoxError) -> Self {
        if error.is::<tower::load_shed::error::Overloaded>() {
            Self::Overloaded
        } else if error.is::<tower::timeout::error::Elapsed>() {
            Self::Timeout
        } else {
            Self::Transport(error.to_string())
        }
    }
    pub(crate) fn transport(error: hyper_util::client::legacy::Error) -> Self {
        Self::Transport(error.to_string())
    }
    pub(crate) fn response_body(_error: impl fmt::Display) -> Self {
        Self::ResponseTooLarge
    }
    pub(crate) fn render_response(status: StatusCode, body: Bytes) -> Self {
        Self::Render {
            status,
            body: String::from_utf8_lossy(&body).into_owned(),
        }
    }
    pub(crate) fn invalid_response(error: serde_json::Error) -> Self {
        Self::InvalidResponse(error.to_string())
    }
    pub(crate) fn health(status: StatusCode) -> Self {
        Self::Transport(format!("SSR health endpoint returned {status}"))
    }
    pub(crate) fn shutdown(status: StatusCode) -> Self {
        Self::Transport(format!("SSR shutdown endpoint returned {status}"))
    }
    pub(crate) fn unavailable() -> Self {
        Self::Unavailable
    }
    /// Returns the stable failure classification.
    pub fn kind(&self) -> crate::SsrFailureKind {
        match self {
            Self::Unavailable => crate::SsrFailureKind::Unavailable,
            Self::Overloaded => crate::SsrFailureKind::Overloaded,
            Self::Timeout => crate::SsrFailureKind::Timeout,
            Self::Transport(_) => crate::SsrFailureKind::Transport,
            Self::InvalidResponse(_) => crate::SsrFailureKind::InvalidResponse,
            Self::Render { .. } => crate::SsrFailureKind::Render,
            Self::ResponseTooLarge => crate::SsrFailureKind::ResponseTooLarge,
            Self::ProcessExited => crate::SsrFailureKind::ProcessExited,
        }
    }
}

impl fmt::Display for SsrFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => formatter.write_str("SSR backend is unavailable"),
            Self::Overloaded => formatter.write_str("SSR backend is overloaded"),
            Self::Timeout => formatter.write_str("SSR render timed out"),
            Self::Transport(message) => write!(formatter, "SSR transport failed: {message}"),
            Self::Render { status, body } => {
                write!(formatter, "SSR server returned {status}: {body}")
            }
            Self::InvalidResponse(error) => write!(formatter, "invalid SSR response: {error}"),
            Self::ResponseTooLarge => {
                formatter.write_str("SSR response exceeded the configured limit")
            }
            Self::ProcessExited => formatter.write_str("SSR process exited"),
        }
    }
}

impl std::error::Error for SsrFailure {}
