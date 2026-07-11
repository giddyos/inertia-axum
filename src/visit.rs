//! Advanced request access for protocol-aware handlers.

use crate::{
    axum::response_headers::{local_uri, request_context},
    RequestContext,
};
use axum::{
    extract::{FromRequestParts, OriginalUri},
    http::{request::Parts, Method},
};
use std::convert::Infallible;

/// Parsed Inertia visit information inserted by the application layer.
#[derive(Clone, Debug)]
pub struct Visit {
    pub(crate) context: RequestContext,
    pub(crate) method: Method,
    pub(crate) uri: Box<str>,
    pub(crate) version: Option<Box<str>>,
    pub(crate) referer: Option<Box<str>>,
}

impl Visit {
    /// Returns whether this request is an Inertia visit.
    pub fn is_inertia(&self) -> bool {
        self.context.is_inertia()
    }
    /// Returns whether partial reload headers are present.
    pub fn is_partial(&self) -> bool {
        self.context.partial_component().is_some()
    }
    /// Returns whether this is a prefetch visit.
    pub fn is_prefetch(&self) -> bool {
        self.context.is_prefetch()
    }
    /// Returns whether this is an explicit reload.
    pub fn is_reload(&self) -> bool {
        self.context.is_reload()
    }
    /// Returns the request asset version.
    pub fn version(&self) -> Option<&str> {
        self.context.version()
    }
    /// Iterates requested partial props without materializing a vector.
    pub fn requested_props(&self) -> impl Iterator<Item = &str> {
        self.context.partial_data_iter()
    }
    /// Iterates excluded partial props without materializing a vector.
    pub fn excluded_props(&self) -> impl Iterator<Item = &str> {
        self.context.partial_except_iter()
    }
    /// Iterates reset props without materializing a vector.
    pub fn reset_props(&self) -> impl Iterator<Item = &str> {
        self.context.reset_iter()
    }
    /// Iterates once props already held by the client.
    pub fn except_once_props(&self) -> impl Iterator<Item = &str> {
        self.context.except_once_props_iter()
    }
    /// Returns the requested validation error bag.
    pub fn error_bag(&self) -> Option<&str> {
        self.context.error_bag()
    }
}

impl<S> FromRequestParts<S> for Visit
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        if let Some(visit) = parts.extensions.get::<Visit>() {
            return Ok(visit.clone());
        }
        Ok(Self {
            context: request_context(&parts.headers),
            method: parts.method.clone(),
            uri: parts
                .extensions
                .get::<OriginalUri>()
                .map(|original| local_uri(&original.0))
                .unwrap_or_else(|| local_uri(&parts.uri)),
            version: None,
            referer: parts
                .headers
                .get(axum::http::header::REFERER)
                .and_then(|value| value.to_str().ok())
                .map(Into::into),
        })
    }
}
