//! Framework-neutral direct and finalized Inertia response values.

use crate::{Location, Redirect, props::PendingProp};
use bytes::Bytes;
use http::{HeaderMap, StatusCode};
use serde::Serialize;
use serde_json::{Map, Value};

/// Body storage for a framework-neutral response.
#[derive(Clone, Debug)]
pub enum CoreBody {
    /// A response with no body.
    Empty,
    /// An owned byte response.
    Bytes(Bytes),
}

/// A finalized response ready for conversion by a framework adapter.
#[derive(Clone, Debug)]
pub struct CoreResponse {
    status: StatusCode,
    headers: HeaderMap,
    body: CoreBody,
}

impl CoreResponse {
    /// Creates a response from its native HTTP parts.
    pub fn new(status: StatusCode, headers: HeaderMap, body: CoreBody) -> Self {
        Self {
            status,
            headers,
            body,
        }
    }

    /// Creates an empty response.
    pub fn empty(status: StatusCode) -> Self {
        Self {
            status,
            headers: HeaderMap::new(),
            body: CoreBody::Empty,
        }
    }

    /// Creates a byte response.
    pub fn bytes(status: StatusCode, body: impl Into<Bytes>) -> Self {
        Self {
            status,
            headers: HeaderMap::new(),
            body: CoreBody::Bytes(body.into()),
        }
    }

    /// Returns the status code.
    pub fn status(&self) -> StatusCode {
        self.status
    }

    /// Returns the response headers.
    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    /// Returns mutable response headers.
    pub fn headers_mut(&mut self) -> &mut HeaderMap {
        &mut self.headers
    }

    /// Consumes the response into its HTTP parts.
    pub fn into_parts(self) -> (StatusCode, HeaderMap, CoreBody) {
        (self.status, self.headers, self.body)
    }
}

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

/// A response awaiting request-aware Inertia finalization.
pub enum PendingResponse {
    /// A dynamic page response.
    Page(Box<PendingPage>),
    /// A method-aware internal redirect.
    Redirect(Redirect),
    /// An external location visit.
    Location(Location),
    /// A semantic form-validation failure.
    InvalidForm(PendingValidation),
}

/// Validation state awaiting transient persistence and redirect-back finalization.
#[derive(Debug)]
#[doc(hidden)]
pub struct PendingValidation {
    pub(crate) errors: Value,
    pub(crate) old_input: Option<Value>,
    pub(crate) back: Box<str>,
}

impl PendingResponse {
    pub(crate) fn uses_transient(&self) -> bool {
        matches!(
            self,
            Self::Page(_) | Self::Redirect(_) | Self::InvalidForm(_)
        )
    }
    pub(crate) fn requires_transient(&self) -> bool {
        match self {
            Self::Page(page) => !page.flash.is_empty(),
            Self::Redirect(redirect) => !redirect.flash.is_empty(),
            Self::Location(_) => false,
            Self::InvalidForm(_) => true,
        }
    }
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

impl From<DynamicPage> for PendingPage {
    fn from(page: DynamicPage) -> Self {
        page.into_pending_page()
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
