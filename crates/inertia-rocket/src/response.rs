//! Rocket-owned wrappers for finalized and pending core responses.

use inertia_core::{CoreBody, CoreResponse, PendingResponse};
use rocket::{
    Request,
    http::Status,
    response::{Responder, Response as RocketResponse},
};
use std::{fmt, io::Cursor, sync::Mutex};

const MISSING_FAIRING: &str = "An Inertia response was returned, but InertiaFairing is not installed.\n\nAttach inertia_rocket::InertiaFairing to the Rocket application.";

/// Rocket response containing a finalized framework-neutral response.
pub struct Response(pub CoreResponse);

/// Descriptive alias for a finalized [`Response`].
pub type InertiaResponse = Response;

/// Result returned by asynchronous Rocket Inertia handlers.
pub type Result = std::result::Result<Response, Error>;

/// Rocket adapter failure that can be returned from a handler.
#[derive(Debug)]
pub struct Error {
    status: Status,
    message: String,
}

impl Error {
    pub(crate) fn internal(error: impl fmt::Display) -> Self {
        Self {
            status: Status::InternalServerError,
            message: error.to_string(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

impl<'r, 'o: 'r> Responder<'r, 'o> for Error {
    fn respond_to(self, request: &'r Request<'_>) -> rocket::response::Result<'o> {
        RocketResponse::build_from(self.message.respond_to(request)?)
            .status(self.status)
            .ok()
    }
}

impl<'r, 'o: 'r> Responder<'r, 'o> for Response {
    fn respond_to(self, _request: &'r Request<'_>) -> rocket::response::Result<'o> {
        core_response(self.0).map_err(|_| Status::InternalServerError)
    }
}

pub(crate) fn core_response(
    response: CoreResponse,
) -> std::result::Result<RocketResponse<'static>, ()> {
    let (status, headers, body) = response.into_parts();
    let mut response = RocketResponse::build();
    response.status(Status::new(status.as_u16()));
    for (name, value) in &headers {
        let value = value.to_str().map_err(|_| ())?;
        response.raw_header_adjoin(name.as_str().to_owned(), value.to_owned());
    }
    match body {
        CoreBody::Empty => {}
        CoreBody::Bytes(bytes) => {
            response.sized_body(bytes.len(), Cursor::new(bytes));
        }
    }
    Ok(response.finalize())
}

#[derive(Default)]
pub(crate) struct PendingSlot(Mutex<Option<PendingResponse>>);

impl PendingSlot {
    pub(crate) fn store(&self, pending: PendingResponse) -> bool {
        let mut slot = self
            .0
            .lock()
            .expect("Rocket pending-response lock poisoned");
        if slot.is_some() {
            return false;
        }
        *slot = Some(pending);
        true
    }

    pub(crate) fn take(&self) -> Option<PendingResponse> {
        self.0
            .lock()
            .expect("Rocket pending-response lock poisoned")
            .take()
    }
}

#[derive(Default)]
pub(crate) struct EarlyResponseSlot(Mutex<Option<CoreResponse>>);

impl EarlyResponseSlot {
    pub(crate) fn store(&self, response: CoreResponse) {
        *self.0.lock().expect("Rocket early-response lock poisoned") = Some(response);
    }

    pub(crate) fn take(&self) -> Option<CoreResponse> {
        self.0
            .lock()
            .expect("Rocket early-response lock poisoned")
            .take()
    }
}

pub(crate) fn pending_response<'r, 'o: 'r>(
    request: &'r Request<'_>,
    pending: PendingResponse,
) -> rocket::response::Result<'o> {
    let slot = request.local_cache(PendingSlot::default);
    if !slot.store(pending) {
        return RocketResponse::build_from(
            "Rocket Inertia request already has a pending response".respond_to(request)?,
        )
        .status(Status::InternalServerError)
        .ok();
    }
    RocketResponse::build_from(MISSING_FAIRING.respond_to(request)?)
        .status(Status::InternalServerError)
        .ok()
}

/// Rocket wrapper for a page awaiting request-aware finalization.
pub struct PendingPage(pub(crate) inertia_core::PendingPage);

impl PendingPage {
    /// Wraps a page produced by a derived typed page.
    pub fn typed(page: impl inertia_core::InertiaPage) -> Self {
        Self(page.into_pending_page())
    }

    /// Converts this wrapper into the core pending page.
    pub fn into_core(self) -> inertia_core::PendingPage {
        self.0
    }
}

impl From<inertia_core::PendingPage> for PendingPage {
    fn from(page: inertia_core::PendingPage) -> Self {
        Self(page)
    }
}

impl<'r, 'o: 'r> Responder<'r, 'o> for PendingPage {
    fn respond_to(self, request: &'r Request<'_>) -> rocket::response::Result<'o> {
        pending_response(request, PendingResponse::Page(Box::new(self.0)))
    }
}

/// Rocket-compatible untyped page builder.
pub struct DynamicPage(inertia_core::DynamicPage);

impl DynamicPage {
    /// Creates a page for `component`.
    pub fn new(component: impl Into<String>) -> Self {
        Self(inertia_core::DynamicPage::new(component))
    }

    /// Serializes and adds a route prop.
    pub fn prop(
        mut self,
        key: impl Into<String>,
        value: impl rocket::serde::Serialize + Send + 'static,
    ) -> Self {
        self.0 = self.0.prop(key, value);
        self
    }

    /// Adds a composable asynchronous prop.
    pub fn async_prop<T>(mut self, key: impl Into<String>, prop: inertia_core::Prop<T>) -> Self
    where
        T: rocket::serde::Serialize + Send + 'static,
    {
        self.0 = self.0.async_prop(key, prop);
        self
    }

    /// Adds an erased prop generated by the portable `page!` macro.
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
    pub fn flash(mut self, key: impl Into<String>, value: impl rocket::serde::Serialize) -> Self {
        self.0 = self.0.flash(key, value);
        self
    }

    /// Converts this page into its pending wrapper.
    pub fn into_pending_page(self) -> PendingPage {
        PendingPage(self.0.into_pending_page())
    }
}

impl<'r, 'o: 'r> Responder<'r, 'o> for DynamicPage {
    fn respond_to(self, request: &'r Request<'_>) -> rocket::response::Result<'o> {
        self.into_pending_page().respond_to(request)
    }
}

/// Rocket-compatible method-aware redirect.
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

    /// Attaches a flash value to the redirect.
    pub fn flash(mut self, key: impl Into<String>, value: impl rocket::serde::Serialize) -> Self {
        self.0 = self.0.flash(key, value);
        self
    }

    /// Converts this wrapper to its core representation.
    pub fn into_core(self) -> inertia_core::Redirect {
        self.0
    }
}

impl From<inertia_core::Redirect> for Redirect {
    fn from(redirect: inertia_core::Redirect) -> Self {
        Self(redirect)
    }
}

impl<'r, 'o: 'r> Responder<'r, 'o> for Redirect {
    fn respond_to(self, request: &'r Request<'_>) -> rocket::response::Result<'o> {
        pending_response(request, PendingResponse::Redirect(self.0))
    }
}

/// Rocket-compatible external location visit.
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
    fn from(location: inertia_core::Location) -> Self {
        Self(location)
    }
}

impl<'r, 'o: 'r> Responder<'r, 'o> for Location {
    fn respond_to(self, request: &'r Request<'_>) -> rocket::response::Result<'o> {
        pending_response(request, PendingResponse::Location(self.0))
    }
}

/// Constructs a Rocket-compatible untyped page with portable prop semantics.
#[macro_export]
macro_rules! page {
    ($component:expr, { $($props:tt)* }) => {{
        let page = $crate::DynamicPage::new($component);
        $crate::__inertia_rocket_page_props!(page; $($props)*)
    }};
}

/// Internal token muncher used by [`page!`](crate::page).
#[doc(hidden)]
#[macro_export]
macro_rules! __inertia_rocket_page_props {
    ($page:ident;) => { $page };
    ($page:ident; $key:ident : $value:expr $(, $($rest:tt)*)?) => {{
        use $crate::__private::IntoPendingProp as _;
        let mut adapter = $crate::__private::DynamicPropAdapter::new($value);
        let prop = adapter.into_pending_prop(stringify!($key).to_owned());
        let $page = $page.pending_prop(prop);
        $crate::__inertia_rocket_page_props!($page; $($($rest)*)?)
    }};
    ($page:ident; $key:ident $(, $($rest:tt)*)?) => {{
        use $crate::__private::IntoPendingProp as _;
        let mut adapter = $crate::__private::DynamicPropAdapter::new($key);
        let prop = adapter.into_pending_prop(stringify!($key).to_owned());
        let $page = $page.pending_prop(prop);
        $crate::__inertia_rocket_page_props!($page; $($($rest)*)?)
    }};
}
