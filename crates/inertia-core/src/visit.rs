//! Advanced request access for protocol-aware handlers.

use crate::{PropKey, RequestContext, RequestParts};
use http::{Method, header::REFERER};

/// Parsed Inertia visit information inserted by the application layer.
#[derive(Clone, Debug)]
pub struct Visit {
    pub(crate) context: RequestContext,
    pub(crate) method: Method,
    pub(crate) uri: Box<str>,
    pub(crate) referer: Option<Box<str>>,
}

impl Visit {
    pub(crate) fn from_request(request: &RequestParts) -> Self {
        let context = RequestContext::from_header_fn(|name| {
            request
                .headers()
                .get(name)
                .and_then(|value| value.to_str().ok())
        });
        Self {
            context,
            method: request.method().clone(),
            uri: request
                .uri()
                .path_and_query()
                .map(|value| value.as_str().into())
                .unwrap_or_else(|| "/".into()),
            referer: request
                .headers()
                .get(REFERER)
                .and_then(|value| value.to_str().ok())
                .map(Into::into),
        }
    }

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
    /// Returns whether a standard eager prop is selected for this visit.
    ///
    /// This advanced escape hatch avoids repository work that must happen
    /// before a typed page can be constructed. Async [`crate::Prop`] resolvers
    /// are selected lazily by the finalizer and do not need it.
    pub fn selects<T>(&self, key: PropKey<T>) -> bool {
        if self.method != Method::GET
            || !self
                .context
                .partial_reload_matches(key.component().as_str())
        {
            return true;
        }
        if self.context.partial_except_iter().next().is_some() {
            return !self
                .context
                .partial_except_iter()
                .any(|name| name == key.name());
        }
        if self.context.partial_data_iter().next().is_some() {
            return self
                .context
                .partial_data_iter()
                .any(|name| name == key.name());
        }
        true
    }
    /// Returns the requested validation error bag.
    pub fn error_bag(&self) -> Option<&str> {
        self.context.error_bag()
    }
}

impl From<RequestParts> for Visit {
    fn from(request: RequestParts) -> Self {
        Self::from_request(&request)
    }
}
