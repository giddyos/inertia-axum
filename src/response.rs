//! Direct Inertia response values and their request-local pending marker.

use crate::{Inertia, Location, Redirect};
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use serde_json::{Map, Value};
use std::sync::{Arc, Mutex};

const MISSING_LAYER: &str = "An Inertia page was returned, but the Inertia layer is not installed.\n\nInstall it on the router:\n\nlet app = Router::new()\n    .route(\"/\", get(index))\n    .inertia(inertia);";

/// A page awaiting request-aware Inertia finalization.
pub struct PendingPage {
    pub(crate) inertia: Inertia<Value>,
    pub(crate) status: StatusCode,
}

/// A response awaiting request-aware Inertia finalization.
pub enum PendingResponse {
    /// A dynamic page response.
    Page(Box<PendingPage>),
    /// A method-aware internal redirect.
    Redirect(Redirect),
    /// An external location visit.
    Location(Location),
}

/// Cloneable request-local one-shot storage for a pending response.
#[derive(Clone)]
pub struct PendingResponseHandle(Arc<Mutex<Option<PendingResponse>>>);

impl PendingResponseHandle {
    /// Stores a pending response in a new handle.
    pub fn new(pending: PendingResponse) -> Self {
        Self(Arc::new(Mutex::new(Some(pending))))
    }

    /// Takes the pending response exactly once.
    pub fn take(&self) -> Option<PendingResponse> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
    }
}

pub(crate) fn pending_response(pending: PendingResponse) -> Response {
    let handle = PendingResponseHandle::new(pending);
    let mut response = (StatusCode::INTERNAL_SERVER_ERROR, MISSING_LAYER).into_response();
    response.extensions_mut().insert(handle);
    response
}

/// An untyped direct page response for small pages and prototypes.
pub struct DynamicPage {
    component: String,
    props: Map<String, Value>,
    encrypt_history: bool,
    clear_history: bool,
    status: StatusCode,
}

impl DynamicPage {
    /// Creates a page for `component`.
    pub fn new(component: impl Into<String>) -> Self {
        Self {
            component: component.into(),
            props: Map::new(),
            encrypt_history: false,
            clear_history: false,
            status: StatusCode::OK,
        }
    }

    /// Serializes and adds a route prop.
    pub fn prop(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        let value = serde_json::to_value(value).expect("DynamicPage prop serialization failed");
        self.props.insert(key.into(), value);
        self
    }

    /// Sets the response status.
    pub fn status(mut self, status: StatusCode) -> Self {
        self.status = status;
        self
    }
    /// Clears encrypted browser history.
    pub fn clear_history(mut self) -> Self {
        self.clear_history = true;
        self
    }
    /// Encrypts browser history for this page.
    pub fn encrypt_history(mut self) -> Self {
        self.encrypt_history = true;
        self
    }

    /// Converts this response into its request-aware pending representation.
    pub fn into_pending_page(self) -> PendingPage {
        let inertia = Inertia::page(self.component).props(Value::Object(self.props));
        // PageMetadata currently has no direct setter on the compatibility
        // builder, so carry phase-1 history modifiers through its public API.
        let inertia = if self.encrypt_history {
            inertia.encrypt_history()
        } else {
            inertia
        };
        let inertia = if self.clear_history {
            inertia.clear_history()
        } else {
            inertia
        };
        PendingPage {
            inertia,
            status: self.status,
        }
    }
}

impl IntoResponse for DynamicPage {
    fn into_response(self) -> Response {
        pending_response(PendingResponse::Page(Box::new(self.into_pending_page())))
    }
}

/// Constructs a [`DynamicPage`] using ordinary serializable values.
#[macro_export]
macro_rules! page {
    ($component:expr, { $($props:tt)* }) => {{
        let page = $crate::DynamicPage::new($component);
        $crate::__inertia_page_props!(page; $($props)*)
    }};
}

/// Internal token muncher used by [`page!`](crate::page).
#[doc(hidden)]
#[macro_export]
macro_rules! __inertia_page_props {
    ($page:ident;) => { $page };
    ($page:ident; $key:ident : $value:expr $(, $($rest:tt)*)?) => {{
        let $page = $page.prop(stringify!($key), $value);
        $crate::__inertia_page_props!($page; $($($rest)*)?)
    }};
    ($page:ident; $key:ident $(, $($rest:tt)*)?) => {{
        let $page = $page.prop(stringify!($key), $key);
        $crate::__inertia_page_props!($page; $($($rest)*)?)
    }};
}
