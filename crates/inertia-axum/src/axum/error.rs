//! Axum-facing serialization, header, and URI errors.

use axum::http::StatusCode;
use axum::http::header::InvalidHeaderValue;
use axum::response::{IntoResponse, Response};
use fluent_uri::ParseError;
use std::error::Error;
use std::fmt;
use tracing::error;

#[derive(Debug)]
/// Error returned by compatibility rendering and finalization helpers.
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
    /// Typed shared-data preparation failed.
    Shared(Box<dyn Error + Send + Sync>),
    /// Flash or redirected errors were used without a transient store.
    MissingTransientStore,
    /// Transient storage failed.
    Transient(Box<dyn Error + Send + Sync>),
    /// Server-side rendering failed in strict mode.
    #[cfg(feature = "ssr")]
    Ssr(crate::ssr::SsrFailure),
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
    pub(crate) fn shared(error: Box<dyn Error + Send + Sync>) -> Self {
        Self::Shared(error)
    }
    pub(crate) fn transient(error: Box<dyn Error + Send + Sync>) -> Self {
        Self::Transient(error)
    }
    #[cfg(feature = "ssr")]
    pub(crate) fn ssr(error: crate::ssr::SsrFailure) -> Self {
        Self::Ssr(error)
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
            Self::Shared(error) => write!(f, "failed to prepare Inertia shared data: {error}"),
            Self::MissingTransientStore => write!(
                f,
                "Inertia flash or redirected error state requires a transient store; configure InertiaAppBuilder::transient(...)"
            ),
            Self::Transient(error) => write!(
                f,
                "failed to load or commit Inertia transient state: {error}"
            ),
            #[cfg(feature = "ssr")]
            Self::Ssr(error) => write!(f, "server-side rendering failed: {error}"),
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
            Self::Shared(error) => Some(error.as_ref()),
            Self::MissingTransientStore => None,
            Self::Transient(error) => Some(error.as_ref()),
            #[cfg(feature = "ssr")]
            Self::Ssr(error) => Some(error),
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
        let actionable = matches!(self, Self::MissingTransientStore).then(|| self.to_string());
        error!(error = %self, "failed to build Axum Inertia response");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            actionable.unwrap_or_else(|| "failed to build Inertia response".to_owned()),
        )
            .into_response()
    }
}

pub(crate) fn internal_error_response(error: InertiaError) -> Response {
    error.into_response()
}
