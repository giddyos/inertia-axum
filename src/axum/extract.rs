//! Axum request extraction and targeted extension lookups.
//!
//! Extraction clones only the extensions needed by Inertia, never the full
//! extension map; rendering methods live in the sibling façade.

use super::response_headers::{local_uri, request_context};
use super::shared::SharedProps;
use super::version::{InertiaVersion, InertiaVersionSource};
use crate::RequestContext;
use axum::extract::{FromRequestParts, OriginalUri};
use axum::http::request::Parts;
use axum::http::Method;
use std::convert::Infallible;
use std::fmt;

pub struct InertiaRequest {
    pub(crate) context: RequestContext,
    pub(crate) method: Method,
    pub(crate) shared_props: Option<SharedProps>,
    pub(crate) uri: Box<str>,
    pub(crate) version: Option<InertiaVersion>,
}

impl fmt::Debug for InertiaRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InertiaRequest")
            .field("context", &self.context)
            .field("method", &self.method)
            .field("uri", &self.uri)
            .field("version", &self.version)
            .field("has_shared_props", &self.shared_props.is_some())
            .finish()
    }
}

impl<S> FromRequestParts<S> for InertiaRequest
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let context = request_context(&parts.headers);
        let version = parts
            .extensions
            .get::<InertiaVersion>()
            .cloned()
            .or_else(|| {
                parts
                    .extensions
                    .get::<InertiaVersionSource>()
                    .map(InertiaVersionSource::resolve)
            });
        let shared_props = parts.extensions.get::<SharedProps>().cloned();
        Ok(Self {
            context,
            method: parts.method.clone(),
            shared_props,
            uri: parts
                .extensions
                .get::<OriginalUri>()
                .map(|original_uri| local_uri(&original_uri.0))
                .unwrap_or_else(|| local_uri(&parts.uri)),
            version,
        })
    }
}
