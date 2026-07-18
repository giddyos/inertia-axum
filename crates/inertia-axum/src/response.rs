//! Axum-owned wrappers around framework-neutral pending and finalized responses.

use axum::{
    body::Body,
    response::{IntoResponse, Response},
};
use inertia_core::{CoreBody, CoreResponse};
use serde::Serialize;
use std::sync::{Arc, Mutex};

const MISSING_LAYER: &str = "An Inertia response was returned, but the Inertia layer is not installed.\n\nInstall it on the router:\n\nlet app = Router::new()\n    .route(\"/\", get(index))\n    .inertia(inertia);\n\n`with_inertia(inertia)` is also available.";

/// Converts a finalized core response into Axum's response body.
pub struct AxumResponse(pub CoreResponse);

impl IntoResponse for AxumResponse {
    fn into_response(self) -> Response {
        let (status, headers, body) = self.0.into_parts();
        let body = match body {
            CoreBody::Empty => Body::empty(),
            CoreBody::Bytes(bytes) => Body::from(bytes),
        };
        let mut response = Response::new(body);
        *response.status_mut() = status;
        *response.headers_mut() = headers;
        response
    }
}

/// Cloneable request-local one-shot storage for a pending core response.
#[derive(Clone)]
pub struct PendingResponseHandle(Arc<Mutex<Option<inertia_core::PendingResponse>>>);

impl PendingResponseHandle {
    /// Stores a pending response in a new handle.
    pub fn new(pending: inertia_core::PendingResponse) -> Self {
        Self(Arc::new(Mutex::new(Some(pending))))
    }

    /// Takes the pending response exactly once.
    pub fn take(&self) -> Option<inertia_core::PendingResponse> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
    }
}

pub(crate) fn pending_response(pending: inertia_core::PendingResponse) -> Response {
    let handle = PendingResponseHandle::new(pending);
    let mut response = (http::StatusCode::INTERNAL_SERVER_ERROR, MISSING_LAYER).into_response();
    response.extensions_mut().insert(handle);
    response
}

/// Axum response wrapper for a request-aware Inertia page.
pub struct PendingPage(pub(crate) inertia_core::PendingPage);

impl PendingPage {
    /// Wraps a page produced by a framework-neutral typed page.
    pub fn typed(page: impl inertia_core::InertiaPage) -> Self {
        Self(page.into_pending_page())
    }

    /// Converts this wrapper back to its core representation.
    pub fn into_core(self) -> inertia_core::PendingPage {
        self.0
    }
}

impl From<inertia_core::PendingPage> for PendingPage {
    fn from(page: inertia_core::PendingPage) -> Self {
        Self(page)
    }
}

impl IntoResponse for PendingPage {
    fn into_response(self) -> Response {
        pending_response(inertia_core::PendingResponse::Page(Box::new(self.0)))
    }
}

/// Axum-compatible untyped page builder.
pub struct DynamicPage(inertia_core::DynamicPage);

impl DynamicPage {
    /// Creates a page for `component`.
    pub fn new(component: impl Into<String>) -> Self {
        Self(inertia_core::DynamicPage::new(component))
    }

    /// Serializes and adds a route prop.
    pub fn prop(mut self, key: impl Into<String>, value: impl Serialize + Send + 'static) -> Self {
        self.0 = self.0.prop(key, value);
        self
    }

    /// Adds a composable asynchronous prop.
    pub fn async_prop<T>(mut self, key: impl Into<String>, prop: inertia_core::Prop<T>) -> Self
    where
        T: Serialize + Send + 'static,
    {
        self.0 = self.0.async_prop(key, prop);
        self
    }

    /// Adds an erased prop produced by the shared core adapter.
    #[doc(hidden)]
    pub fn pending_prop(mut self, prop: inertia_core::__private::PendingProp) -> Self {
        self.0 = self.0.pending_prop(prop);
        self
    }

    /// Sets the response status.
    pub fn status(mut self, status: http::StatusCode) -> Self {
        self.0 = self.0.status(status);
        self
    }

    /// Clears encrypted browser history.
    pub fn clear_history(mut self) -> Self {
        self.0 = self.0.clear_history();
        self
    }

    /// Encrypts browser history for this page.
    pub fn encrypt_history(mut self) -> Self {
        self.0 = self.0.encrypt_history();
        self
    }

    /// Preserves the original URL fragment across a redirect.
    pub fn preserve_fragment(mut self) -> Self {
        self.0 = self.0.preserve_fragment();
        self
    }

    /// Attaches a flash value to this page.
    pub fn flash(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        self.0 = self.0.flash(key, value);
        self
    }

    /// Converts this page into its request-aware wrapper.
    pub fn into_pending_page(self) -> PendingPage {
        PendingPage(self.0.into_pending_page())
    }
}

impl IntoResponse for DynamicPage {
    fn into_response(self) -> Response {
        self.into_pending_page().into_response()
    }
}

/// Axum-compatible method-aware redirect.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Redirect(inertia_core::Redirect);

impl Redirect {
    /// Creates a redirect to `url`.
    pub fn new(url: impl Into<String>) -> Self {
        Self(inertia_core::Redirect::new(url))
    }

    /// Creates a redirect to a concrete destination.
    pub fn to(url: impl Into<String>) -> Self {
        Self::new(url)
    }

    /// Creates a redirect to the request referrer.
    pub fn back() -> Self {
        Self(inertia_core::Redirect::back())
    }

    /// Creates a redirect to the request referrer with a fallback.
    pub fn back_or(url: impl Into<String>) -> Self {
        Self(inertia_core::Redirect::back_or(url))
    }

    /// Returns the fallback or concrete destination.
    pub fn url(&self) -> &str {
        self.0.url()
    }

    /// Flashes a value to the page reached by the redirect.
    pub fn flash(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        self.0 = self.0.flash(key, value);
        self
    }

    /// Converts this wrapper to its core representation.
    pub fn into_core(self) -> inertia_core::Redirect {
        self.0
    }
}

impl From<inertia_core::Redirect> for Redirect {
    fn from(value: inertia_core::Redirect) -> Self {
        Self(value)
    }
}

impl IntoResponse for Redirect {
    fn into_response(self) -> Response {
        pending_response(inertia_core::PendingResponse::Redirect(self.0))
    }
}

/// Axum-compatible external location visit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Location(inertia_core::Location);

impl Location {
    /// Creates an external location visit.
    pub fn external(url: impl Into<String>) -> Self {
        Self(inertia_core::Location::external(url))
    }

    /// Creates an external location visit.
    pub fn new(url: impl Into<String>) -> Self {
        Self(inertia_core::Location::new(url))
    }

    /// Returns the destination URL.
    pub fn url(&self) -> &str {
        self.0.url()
    }

    /// Converts this wrapper to its core representation.
    pub fn into_core(self) -> inertia_core::Location {
        self.0
    }
}

impl From<inertia_core::Location> for Location {
    fn from(value: inertia_core::Location) -> Self {
        Self(value)
    }
}

impl IntoResponse for Location {
    fn into_response(self) -> Response {
        pending_response(inertia_core::PendingResponse::Location(self.0))
    }
}

/// Constructs an Axum-compatible [`DynamicPage`] from serializable props.
#[macro_export]
macro_rules! page {
    ($component:expr, { $($props:tt)* }) => {{
        let page = $crate::DynamicPage::new($component);
        $crate::__inertia_axum_page_props!(page; $($props)*)
    }};
}

/// Internal token muncher used by [`page!`](crate::page).
#[doc(hidden)]
#[macro_export]
macro_rules! __inertia_axum_page_props {
    ($page:ident;) => { $page };
    ($page:ident; $key:ident : $value:expr $(, $($rest:tt)*)?) => {{
        use $crate::__private::IntoPendingProp as _;
        let mut adapter = $crate::__private::DynamicPropAdapter::new($value);
        let prop = adapter.into_pending_prop(stringify!($key).to_owned());
        let $page = $page.pending_prop(prop);
        $crate::__inertia_axum_page_props!($page; $($($rest)*)?)
    }};
    ($page:ident; $key:ident $(, $($rest:tt)*)?) => {{
        use $crate::__private::IntoPendingProp as _;
        let mut adapter = $crate::__private::DynamicPropAdapter::new($key);
        let prop = adapter.into_pending_prop(stringify!($key).to_owned());
        let $page = $page.pending_prop(prop);
        $crate::__inertia_axum_page_props!($page; $($($rest)*)?)
    }};
}
