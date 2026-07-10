//! Request-aware shared-prop providers for Axum.
//!
//! Providers are stored in `Arc<Vec<_>>` and registrations use `Arc::make_mut`;
//! merge order, blocked-root skipping, and optional omission semantics remain
//! unchanged.

use crate::page::PageDraft;
use crate::RequestContext;
use axum::http::Method;
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;

type SharedPropProvider = Arc<
    dyn for<'a> Fn(&SharedRequest<'a>) -> Result<Option<Value>, serde_json::Error> + Send + Sync,
>;

#[derive(Clone, Default)]
pub struct SharedProps {
    providers: Arc<Vec<(Box<str>, SharedPropProvider)>>,
}

impl SharedProps {
    /// Creates an empty shared prop registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an empty shared-prop registry with space for `capacity` entries.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            providers: Arc::new(Vec::with_capacity(capacity)),
        }
    }

    /// Registers a fixed serializable shared prop value.
    pub fn value<K, T>(mut self, key: K, value: T) -> Self
    where
        K: Into<Box<str>>,
        T: Send + Sync + Serialize + 'static,
    {
        let provider =
            Arc::new(move |_request: &SharedRequest<'_>| serde_json::to_value(&value).map(Some));
        Arc::make_mut(&mut self.providers).push((key.into(), provider));
        self
    }

    /// Registers a request-aware shared prop provider.
    ///
    /// The provider should return an owned serializable value. For values read
    /// from request extensions, clone the value before returning it.
    pub fn prop<K, F, T>(mut self, key: K, provider: F) -> Self
    where
        K: Into<Box<str>>,
        F: for<'a> Fn(&SharedRequest<'a>) -> T + Send + Sync + 'static,
        T: Serialize,
    {
        let provider = Arc::new(move |request: &SharedRequest<'_>| {
            serde_json::to_value(provider(request)).map(Some)
        });

        Arc::make_mut(&mut self.providers).push((key.into(), provider));
        self
    }

    /// Registers a request-aware shared prop provider that can skip its key.
    ///
    /// Returning `None` omits the shared prop instead of serializing it as
    /// JSON `null`.
    pub fn prop_optional<K, F, T>(mut self, key: K, provider: F) -> Self
    where
        K: Into<Box<str>>,
        F: for<'a> Fn(&SharedRequest<'a>) -> Option<T> + Send + Sync + 'static,
        T: Serialize,
    {
        let provider = Arc::new(move |request: &SharedRequest<'_>| {
            provider(request).map(serde_json::to_value).transpose()
        });

        Arc::make_mut(&mut self.providers).push((key.into(), provider));
        self
    }

    /// Returns `true` when no shared props have been registered.
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    pub(crate) fn merge_into(
        &self,
        request: &SharedRequest<'_>,
        page: &mut PageDraft,
    ) -> Result<(), serde_json::Error> {
        for (key, provider) in self.providers.iter() {
            if page.global_is_blocked(key) {
                continue;
            }

            if let Some(value) = provider(request)? {
                page.insert_global_shared(key, value);
            }
        }

        Ok(())
    }
}

/// Narrow request view available to global shared-prop providers.
pub struct SharedRequest<'a> {
    context: &'a RequestContext,
    method: &'a Method,
    uri: &'a str,
    asset_version: Option<&'a str>,
}

impl<'a> SharedRequest<'a> {
    pub(crate) fn new(
        context: &'a RequestContext,
        method: &'a Method,
        uri: &'a str,
        asset_version: Option<&'a str>,
    ) -> Self {
        Self {
            context,
            method,
            uri,
            asset_version,
        }
    }

    /// Returns parsed Inertia request headers.
    pub fn context(&self) -> &RequestContext {
        self.context
    }

    /// Returns the request method.
    pub fn method(&self) -> &Method {
        self.method
    }

    /// Returns the local request URI.
    pub fn uri(&self) -> &str {
        self.uri
    }

    /// Returns the resolved asset version, if any.
    pub fn asset_version(&self) -> Option<&str> {
        self.asset_version
    }
}
