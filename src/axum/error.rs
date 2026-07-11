//! Axum-facing serialization, header, and URI errors.

use axum::http::header::InvalidHeaderValue;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use fluent_uri::ParseError;
use std::error::Error;
use std::fmt;
use tracing::error;

#[derive(Debug)]
pub enum InertiaError {
    /// The page object could not be serialized.
    Serialization(serde_json::Error),
    /// A response header value could not be constructed.
    InvalidHeader(InvalidHeaderValue),
    /// A redirect or location URL was not a valid URI reference.
    InvalidUri(ParseError),
    /// The application root view could not be rendered.
    Root(Box<dyn Error + Send + Sync>),
    /// An asynchronous prop resolver failed.
    Prop(crate::PropError),
}

impl InertiaError {
    pub(crate) fn invalid_header(error: InvalidHeaderValue) -> Self {
        Self::InvalidHeader(error)
    }

    pub(crate) fn invalid_uri(error: ParseError) -> Self {
        Self::InvalidUri(error)
    }

    pub(crate) fn root(error: Box<dyn Error + Send + Sync>) -> Self {
        Self::Root(error)
    }
    pub(crate) fn prop(error: crate::PropError) -> Self {
        Self::Prop(error)
    }
}

impl fmt::Display for InertiaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Serialization(error) => write!(f, "failed to serialize Inertia page: {error}"),
            Self::InvalidHeader(error) => write!(f, "invalid Inertia response header: {error}"),
            Self::InvalidUri(error) => write!(f, "invalid Inertia URI reference: {error}"),
            Self::Root(error) => write!(f, "failed to render Inertia root view: {error}"),
            Self::Prop(error) => write!(f, "failed to resolve Inertia prop: {error}"),
        }
    }
}

impl Error for InertiaError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Serialization(error) => Some(error),
            Self::InvalidHeader(error) => Some(error),
            Self::InvalidUri(error) => Some(error),
            Self::Root(error) => Some(error.as_ref()),
            Self::Prop(error) => Some(error),
        }
    }
}

impl From<serde_json::Error> for InertiaError {
    fn from(error: serde_json::Error) -> Self {
        Self::Serialization(error)
    }
}

impl IntoResponse for InertiaError {
    fn into_response(self) -> Response {
        error!(error = %self, "failed to build Axum Inertia response");

        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to build Inertia response",
        )
            .into_response()
    }
}

pub(crate) fn internal_error_response(error: InertiaError) -> Response {
    error.into_response()
}
