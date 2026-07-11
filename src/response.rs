//! Direct Inertia response values and their request-local pending marker.

use crate::{props::PendingProp, Location, Redirect};
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
    pub(crate) component: String,
    pub(crate) props: Vec<PendingProp>,
    pub(crate) encrypt_history: bool,
    pub(crate) clear_history: bool,
    pub(crate) preserve_fragment: bool,
    pub(crate) flash: Map<String, Value>,
    pub(crate) status: StatusCode,
}

impl IntoResponse for PendingPage {
    fn into_response(self) -> Response {
        pending_response(PendingResponse::Page(Box::new(self)))
    }
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

impl PendingResponse {
    pub(crate) fn uses_transient(&self) -> bool {
        matches!(self, Self::Page(_) | Self::Redirect(_))
    }
    pub(crate) fn requires_transient(&self) -> bool {
        match self {
            Self::Page(page) => !page.flash.is_empty(),
            Self::Redirect(redirect) => !redirect.flash.is_empty(),
            Self::Location(_) => false,
        }
    }
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
    props: Vec<PendingProp>,
    encrypt_history: bool,
    clear_history: bool,
    preserve_fragment: bool,
    flash: Map<String, Value>,
    status: StatusCode,
}

impl DynamicPage {
    /// Creates a page for `component`.
    pub fn new(component: impl Into<String>) -> Self {
        Self {
            component: component.into(),
            props: Vec::new(),
            encrypt_history: false,
            clear_history: false,
            preserve_fragment: false,
            flash: Map::new(),
            status: StatusCode::OK,
        }
    }

    /// Serializes and adds a route prop.
    pub fn prop(mut self, key: impl Into<String>, value: impl Serialize + Send + 'static) -> Self {
        use crate::props::prop::IntoPendingProp as _;
        let mut adapter = crate::props::prop::DynamicPropAdapter::new(value);
        self.props
            .push((&mut adapter).into_pending_prop(key.into()));
        self
    }

    /// Adds an already erased prop. This is used by [`page!`](crate::page).
    #[doc(hidden)]
    pub fn pending_prop(mut self, prop: PendingProp) -> Self {
        self.props.push(prop);
        self
    }

    /// Adds a composable asynchronous prop through the ordinary builder API.
    pub fn async_prop<T>(mut self, key: impl Into<String>, prop: crate::Prop<T>) -> Self
    where
        T: Serialize + Send + 'static,
    {
        use crate::props::prop::IntoPendingProp as _;
        self.props
            .push(crate::props::prop::DynamicPropAdapter::new(prop).into_pending_prop(key.into()));
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
    /// Preserves the original URL fragment across a redirect.
    pub fn preserve_fragment(mut self) -> Self {
        self.preserve_fragment = true;
        self
    }
    /// Attaches a flash value to this page outside the props/history namespace.
    pub fn flash(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        self.flash.insert(
            key.into(),
            serde_json::to_value(value).expect("DynamicPage flash serialization failed"),
        );
        self
    }

    /// Converts this response into its request-aware pending representation.
    pub fn into_pending_page(self) -> PendingPage {
        PendingPage {
            component: self.component,
            props: self.props,
            encrypt_history: self.encrypt_history,
            clear_history: self.clear_history,
            preserve_fragment: self.preserve_fragment,
            flash: self.flash,
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
        use $crate::__private::IntoPendingProp as _;
        let mut adapter = $crate::__private::DynamicPropAdapter::new($value);
        let prop = adapter.into_pending_prop(stringify!($key).to_owned());
        let $page = $page.pending_prop(prop);
        $crate::__inertia_page_props!($page; $($($rest)*)?)
    }};
    ($page:ident; $key:ident $(, $($rest:tt)*)?) => {{
        use $crate::__private::IntoPendingProp as _;
        let mut adapter = $crate::__private::DynamicPropAdapter::new($key);
        let prop = adapter.into_pending_prop(stringify!($key).to_owned());
        let $page = $page.pending_prop(prop);
        $crate::__inertia_page_props!($page; $($($rest)*)?)
    }};
}
