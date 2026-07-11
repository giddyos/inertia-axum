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
        }
    }
}

impl std::error::Error for SsrStartError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidEndpoint { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub(crate) enum SsrFailure {
    Request(String),
    Service(String),
    Transport(String),
    ResponseBody(String),
    Render { status: StatusCode, body: Bytes },
    InvalidResponse(serde_json::Error),
    Health(StatusCode),
    Shutdown(StatusCode),
}

impl SsrFailure {
    pub(crate) fn request(error: axum::http::Error) -> Self {
        Self::Request(error.to_string())
    }
    pub(crate) fn service(error: tower::BoxError) -> Self {
        Self::Service(error.to_string())
    }
    pub(crate) fn transport(error: hyper_util::client::legacy::Error) -> Self {
        Self::Transport(error.to_string())
    }
    pub(crate) fn response_body(error: impl fmt::Display) -> Self {
        Self::ResponseBody(error.to_string())
    }
    pub(crate) fn render_response(status: StatusCode, body: Bytes) -> Self {
        Self::Render { status, body }
    }
    pub(crate) fn invalid_response(error: serde_json::Error) -> Self {
        Self::InvalidResponse(error)
    }
    pub(crate) fn health(status: StatusCode) -> Self {
        Self::Health(status)
    }
    pub(crate) fn shutdown(status: StatusCode) -> Self {
        Self::Shutdown(status)
    }
}

impl fmt::Display for SsrFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Request(message) => write!(formatter, "failed to build SSR request: {message}"),
            Self::Service(message) => write!(formatter, "SSR render service failed: {message}"),
            Self::Transport(message) => write!(formatter, "SSR transport failed: {message}"),
            Self::ResponseBody(message) => {
                write!(formatter, "failed to read SSR response: {message}")
            }
            Self::Render { status, body } => write!(
                formatter,
                "SSR server returned {status}: {}",
                String::from_utf8_lossy(body)
            ),
            Self::InvalidResponse(error) => write!(formatter, "invalid SSR response: {error}"),
            Self::Health(status) => write!(formatter, "SSR health endpoint returned {status}"),
            Self::Shutdown(status) => write!(formatter, "SSR shutdown endpoint returned {status}"),
        }
    }
}

impl std::error::Error for SsrFailure {}
