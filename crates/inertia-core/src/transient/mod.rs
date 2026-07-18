//! Lazy request-to-request flash and validation transport.

#[cfg(feature = "cookies")]
mod cookie;
mod memory;
#[cfg(feature = "tower-sessions")]
mod tower_session;

#[cfg(feature = "cookies")]
pub use cookie::CookieTransient;
pub use memory::MemoryTransient;
#[cfg(feature = "tower-sessions")]
pub use tower_session::TowerSessionTransient;

use crate::{CoreResponse, RequestParts};
use http::{HeaderValue, Method, Uri, header::COOKIE};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{error::Error, future::Future, pin::Pin, sync::Arc};

/// Narrow owned request projection retained until transient state is needed.
#[derive(Clone, Debug)]
pub(crate) struct TransientSeed {
    method: Method,
    uri: Uri,
    cookie: Option<HeaderValue>,
    test_scope: Option<Box<str>>,
    #[cfg(feature = "tower-sessions")]
    session: Option<tower_sessions::Session>,
}

impl TransientSeed {
    pub(crate) fn capture(request: &RequestParts) -> Self {
        Self {
            method: request.method().clone(),
            uri: request.uri().clone(),
            cookie: request.headers().get(COOKIE).cloned(),
            test_scope: request
                .headers()
                .get("x-inertia-transient-id")
                .and_then(|value| value.to_str().ok())
                .map(Into::into),
            #[cfg(feature = "tower-sessions")]
            session: None,
        }
    }

    #[cfg(feature = "tower-sessions")]
    pub(crate) fn with_tower_session(mut self, session: Option<tower_sessions::Session>) -> Self {
        self.session = session;
        self
    }
    pub(crate) fn request(&self) -> TransientRequest<'_> {
        TransientRequest {
            method: &self.method,
            uri: &self.uri,
            cookie: self.cookie.as_ref(),
            test_scope: self.test_scope.as_deref(),
            #[cfg(feature = "tower-sessions")]
            session: self.session.as_ref(),
        }
    }
}

/// Borrowed request data available to transient stores.
#[derive(Clone, Copy)]
pub struct TransientRequest<'a> {
    method: &'a Method,
    uri: &'a Uri,
    cookie: Option<&'a HeaderValue>,
    test_scope: Option<&'a str>,
    #[cfg(feature = "tower-sessions")]
    session: Option<&'a tower_sessions::Session>,
}

impl TransientRequest<'_> {
    /// Returns the request method.
    pub fn method(&self) -> &Method {
        self.method
    }
    /// Returns the request URI.
    pub fn uri(&self) -> &Uri {
        self.uri
    }
    /// Returns the raw Cookie header when present.
    pub fn cookie_header(&self) -> Option<&HeaderValue> {
        self.cookie
    }
    /// Returns the explicit in-memory test scope.
    pub fn test_scope(&self) -> Option<&str> {
        self.test_scope
    }
    /// Returns the explicitly projected tower session.
    #[cfg(feature = "tower-sessions")]
    pub fn tower_session(&self) -> Option<&tower_sessions::Session> {
        self.session
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(crate) struct StoredTransient {
    #[serde(default)]
    flash: Map<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    errors: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    old_input: Option<Value>,
}

/// Loaded and outgoing one-request transient state.
#[derive(Debug, Default)]
pub struct TransientData {
    incoming: StoredTransient,
    outgoing: StoredTransient,
    scope: Box<str>,
    #[cfg(feature = "tower-sessions")]
    session: Option<tower_sessions::Session>,
}

impl TransientData {
    pub(crate) fn loaded(stored: StoredTransient, scope: impl Into<Box<str>>) -> Self {
        Self {
            incoming: stored,
            outgoing: StoredTransient::default(),
            scope: scope.into(),
            #[cfg(feature = "tower-sessions")]
            session: None,
        }
    }
    pub(crate) fn scope(&self) -> &str {
        &self.scope
    }
    pub(crate) fn incoming_flash(&self) -> &Map<String, Value> {
        &self.incoming.flash
    }
    pub(crate) fn flash_next_value(&mut self, key: String, value: Value) {
        self.outgoing.flash.insert(key, value);
    }
    /// Stores serialized validation errors for the next request.
    pub fn store_errors(&mut self, errors: Value) {
        self.outgoing.errors = Some(errors);
    }
    /// Borrows errors loaded for this request.
    pub fn errors(&self) -> Option<&Value> {
        self.incoming.errors.as_ref()
    }
    /// Stores explicitly opted-in redacted old input for the next request.
    pub fn store_old_input(&mut self, input: Value) {
        self.outgoing.old_input = Some(input);
    }
    /// Borrows old input loaded for this request.
    pub fn old_input(&self) -> Option<&Value> {
        self.incoming.old_input.as_ref()
    }
    /// Reflashes all unconsumed incoming values to the next request.
    pub fn reflash(&mut self) {
        self.outgoing.flash.extend(self.incoming.flash.clone());
        if self.outgoing.errors.is_none() {
            self.outgoing.errors.clone_from(&self.incoming.errors);
        }
        if self.outgoing.old_input.is_none() {
            self.outgoing.old_input.clone_from(&self.incoming.old_input);
        }
    }
    pub(crate) fn into_stored(self) -> StoredTransient {
        self.outgoing
    }
    #[cfg(feature = "tower-sessions")]
    pub(crate) fn with_session(mut self, session: tower_sessions::Session) -> Self {
        self.session = Some(session);
        self
    }
    #[cfg(feature = "tower-sessions")]
    pub(crate) fn into_session_parts(self) -> (StoredTransient, Option<tower_sessions::Session>) {
        (self.outgoing, self.session)
    }
}

/// Pluggable request-to-request state storage.
pub trait TransientStore: Clone + Send + Sync + 'static {
    /// Storage failure.
    type Error: Error + Send + Sync + 'static;
    /// Loads and consumes state for this request.
    fn load(
        &self,
        request: TransientRequest<'_>,
    ) -> impl Future<Output = Result<TransientData, Self::Error>> + Send;
    /// Commits outgoing state to the response.
    fn commit(
        &self,
        response: &mut CoreResponse,
        data: TransientData,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;
}

type BoxError = Box<dyn Error + Send + Sync>;
type StoreFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, BoxError>> + Send + 'a>>;

pub(crate) trait ErasedTransientStore: Send + Sync {
    fn load<'a>(&'a self, request: TransientRequest<'a>) -> StoreFuture<'a, TransientData>;
    fn commit<'a>(
        &'a self,
        response: &'a mut CoreResponse,
        data: TransientData,
    ) -> StoreFuture<'a, ()>;
}

impl<T: TransientStore> ErasedTransientStore for T {
    fn load<'a>(&'a self, request: TransientRequest<'a>) -> StoreFuture<'a, TransientData> {
        Box::pin(async move {
            TransientStore::load(self, request)
                .await
                .map_err(|error| Box::new(error) as _)
        })
    }
    fn commit<'a>(
        &'a self,
        response: &'a mut CoreResponse,
        data: TransientData,
    ) -> StoreFuture<'a, ()> {
        Box::pin(async move {
            TransientStore::commit(self, response, data)
                .await
                .map_err(|error| Box::new(error) as _)
        })
    }
}

pub(crate) type SharedTransientStore = Arc<dyn ErasedTransientStore>;
