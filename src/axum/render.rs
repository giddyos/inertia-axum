//! Axum integration for `inertia_axum`.

use crate::html::html_response_context;
use crate::page::PageDraft;
use crate::{
    Inertia, IntoPageProps, Location, Redirect, RequestContext, VARY, X_INERTIA, X_INERTIA_HEADER,
    X_INERTIA_LOCATION_HEADER, X_INERTIA_REDIRECT_HEADER,
};
#[cfg(test)]
use crate::{X_INERTIA_LOCATION, X_INERTIA_REDIRECT};
use ::axum::extract::{FromRequestParts, OriginalUri};
use ::axum::http::header::{InvalidHeaderValue, LOCATION, VARY as VARY_HEADER};
use ::axum::http::request::Parts;
use ::axum::http::uri::Uri;
use ::axum::http::{HeaderMap, HeaderValue, Method, Request, StatusCode};
use ::axum::response::{IntoResponse, Response};
use ::axum::Json;
use fluent_uri::{ParseError, UriRef};
use pin_project_lite::pin_project;
use serde::Serialize;
use serde_json::Value;
use std::convert::Infallible;
use std::error::Error;
use std::fmt;
use std::sync::Arc;
use std::task::{Context, Poll};
use tower_layer::Layer;
use tower_service::Service;
use tracing::error;

pub use crate::HtmlResponseContext;

type SharedPropProvider = Arc<
    dyn for<'a> Fn(&SharedRequest<'a>) -> Result<Option<Value>, serde_json::Error> + Send + Sync,
>;
type VersionProvider = Arc<dyn Fn() -> Arc<str> + Send + Sync>;

#[derive(Clone)]
enum VersionSource {
    Static(Arc<str>),
    Dynamic(VersionProvider),
}

impl VersionSource {
    fn resolve(&self) -> Arc<str> {
        match self {
            Self::Static(version) => Arc::clone(version),
            Self::Dynamic(provider) => provider(),
        }
    }
}

pin_project! {
    #[project = VersionFutureProj]
    pub enum VersionFuture<F, E> {
        Inner { #[pin] future: F },
        Ready { result: Option<Result<Response, E>> },
    }
}

impl<F, E> std::future::Future for VersionFuture<F, E>
where
    F: std::future::Future<Output = Result<Response, E>>,
{
    type Output = Result<Response, E>;

    fn poll(self: std::pin::Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project() {
            VersionFutureProj::Inner { future } => future.poll(context),
            VersionFutureProj::Ready { result } => Poll::Ready(
                result
                    .take()
                    .expect("ready version future polled after completion"),
            ),
        }
    }
}

fn header<'headers>(headers: &'headers HeaderMap, name: &str) -> Option<&'headers str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

fn request_context(headers: &HeaderMap) -> RequestContext {
    RequestContext::from_header_fn(|name| header(headers, name))
}

fn add_vary_header(response: &mut Response) {
    let has_inertia = response
        .headers()
        .get_all(VARY_HEADER)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .any(|value| value.eq_ignore_ascii_case(X_INERTIA));

    if !has_inertia {
        response
            .headers_mut()
            .append(VARY, HeaderValue::from_static(X_INERTIA));
    }
}

fn is_write_method(method: &Method) -> bool {
    matches!(
        *method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    )
}

fn location_header(url: &str) -> Result<HeaderValue, InertiaError> {
    let (header, _has_fragment) = location_header_with_fragment(url)?;
    Ok(header)
}

fn location_header_with_fragment(url: &str) -> Result<(HeaderValue, bool), InertiaError> {
    let uri = UriRef::parse(url).map_err(InertiaError::invalid_uri)?;
    let has_fragment = uri.fragment().is_some();
    HeaderValue::from_str(url)
        .map_err(InertiaError::invalid_header)
        .map(|header| (header, has_fragment))
}

fn local_uri(uri: &Uri) -> String {
    uri.path_and_query()
        .map(|path_and_query| path_and_query.as_str().to_owned())
        .unwrap_or_else(|| "/".to_owned())
}

fn original_uri_from_extensions<B>(request: &Request<B>) -> String {
    request
        .extensions()
        .get::<OriginalUri>()
        .map(|original_uri| local_uri(&original_uri.0))
        .unwrap_or_else(|| local_uri(request.uri()))
}

fn redirect_response(status: StatusCode, url: &str) -> Result<Response, InertiaError> {
    let mut response = status.into_response();
    response
        .headers_mut()
        .insert(LOCATION, location_header(url)?);
    add_vary_header(&mut response);
    Ok(response)
}

fn conflict_response(url: &str) -> Result<Response, InertiaError> {
    let mut response = StatusCode::CONFLICT.into_response();
    let (location, has_fragment) = location_header_with_fragment(url)?;
    let header = if has_fragment {
        X_INERTIA_REDIRECT_HEADER
    } else {
        X_INERTIA_LOCATION_HEADER
    };
    response.headers_mut().insert(header, location);
    add_vary_header(&mut response);
    Ok(response)
}

/// Error returned while building Axum Inertia responses.
#[derive(Debug)]
pub enum InertiaError {
    /// The page object could not be serialized.
    Serialization(serde_json::Error),
    /// A response header value could not be constructed.
    InvalidHeader(InvalidHeaderValue),
    /// A redirect or location URL was not a valid URI reference.
    InvalidUri(ParseError),
}

impl InertiaError {
    fn invalid_header(error: InvalidHeaderValue) -> Self {
        Self::InvalidHeader(error)
    }

    fn invalid_uri(error: ParseError) -> Self {
        Self::InvalidUri(error)
    }
}

impl fmt::Display for InertiaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Serialization(error) => write!(f, "failed to serialize Inertia page: {error}"),
            Self::InvalidHeader(error) => write!(f, "invalid Inertia response header: {error}"),
            Self::InvalidUri(error) => write!(f, "invalid Inertia URI reference: {error}"),
        }
    }
}

impl Error for InertiaError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Serialization(error) => Some(error),
            Self::InvalidHeader(error) => Some(error),
            Self::InvalidUri(error) => Some(error),
        }
    }
}

impl From<serde_json::Error> for InertiaError {
    fn from(error: serde_json::Error) -> Self {
        Self::Serialization(error)
    }
}

impl IntoResponse for InertiaError {
    fn into_response(self) -> Response {
        error!(error = %self, "failed to build Axum Inertia response");

        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to build Inertia response",
        )
            .into_response()
    }
}

fn internal_error_response(error: InertiaError) -> Response {
    error.into_response()
}

/// Current asset version inserted by [`VersionLayer`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InertiaVersion(Arc<str>);

impl InertiaVersion {
    /// Creates an asset version value for request extensions.
    pub fn new<V: Into<Arc<str>>>(version: V) -> Self {
        Self(version.into())
    }

    /// Returns the asset version string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn clone_arc(&self) -> Arc<str> {
        Arc::clone(&self.0)
    }
}

#[derive(Clone)]
struct InertiaVersionSource(VersionSource);

impl InertiaVersionSource {
    fn resolve(&self) -> InertiaVersion {
        InertiaVersion::new(self.0.resolve())
    }
}

/// Shared Inertia props resolved for every Axum page response.
///
/// Register this as an Axum extension layer with [`axum::Extension`]. Shared
/// props are shallow-merged into page props; route props win on key collisions.
/// Providers run once per page response and may inspect the extracted
/// [`InertiaRequest`]. Dotted keys, such as `auth.user`, are expanded into
/// nested props.
///
/// Shared props are merged after partial-reload filtering, so they remain
/// present on partial responses even when omitted from `only` or `except`
/// reload options.
///
/// ```rust,no_run
/// use axum::{Extension, Router};
/// use inertia_axum::axum::{SharedProps, VersionLayer};
///
/// let shared_props = SharedProps::new()
///     .value("appName", "My App")
///     .prop_optional("auth.csrfToken", |request| {
///         request.context().is_inertia().then_some("csrf-token")
///     });
///
/// let app: Router<()> = Router::new()
///     .layer(Extension(shared_props))
///     .layer(VersionLayer::new("asset-version-1"));
/// ```
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

    fn merge_into(
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
    fn new(
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

/// Axum request extractor for Inertia protocol state.
///
/// Use this extractor in handlers that return Inertia page responses. Pair it
/// with [`VersionLayer`] when page objects should include an asset version and
/// stale Inertia visits should receive a `409 Conflict` response.
pub struct InertiaRequest {
    context: RequestContext,
    method: Method,
    shared_props: Option<SharedProps>,
    uri: String,
    version: Option<InertiaVersion>,
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

impl InertiaRequest {
    /// Returns `true` when the request includes the `X-Inertia` header.
    pub fn is_inertia(&self) -> bool {
        self.context.is_inertia()
    }

    /// Returns the parsed request context.
    pub fn context(&self) -> &RequestContext {
        &self.context
    }

    /// Returns the request URI used as the default page-object URL.
    pub fn uri(&self) -> &str {
        &self.uri
    }

    /// Returns the current asset version installed by [`VersionLayer`].
    pub fn asset_version(&self) -> Option<&str> {
        self.version.as_ref().map(InertiaVersion::as_str)
    }

    /// Converts an [`Inertia`] value into an Axum response.
    ///
    /// Inertia requests receive a JSON page object with `X-Inertia: true`.
    /// Direct browser requests are rendered through `html_response`.
    pub fn render<T, F, R>(
        &self,
        inertia: Inertia<T>,
        html_response: F,
    ) -> Result<Response, InertiaError>
    where
        T: IntoPageProps,
        F: FnOnce(HtmlResponseContext) -> R,
        R: IntoResponse,
    {
        let partial_reload_enabled = self.method == Method::GET;
        let mut draft = inertia.into_page_draft(
            &self.uri,
            self.version.as_ref().map(InertiaVersion::clone_arc),
            &self.context,
            partial_reload_enabled,
        )?;

        if let Some(shared_props) = &self.shared_props {
            if !shared_props.is_empty() {
                let shared_request = SharedRequest::new(
                    &self.context,
                    &self.method,
                    &self.uri,
                    self.asset_version(),
                );
                shared_props.merge_into(&shared_request, &mut draft)?;
            }
        }

        let page = draft.finish();

        if self.context.is_inertia() {
            let mut response = Json(page).into_response();
            response
                .headers_mut()
                .insert(X_INERTIA_HEADER, HeaderValue::from_static("true"));
            add_vary_header(&mut response);
            Ok(response)
        } else {
            let context = html_response_context(&page)?;
            let mut response = html_response(context).into_response();
            add_vary_header(&mut response);
            Ok(response)
        }
    }

    /// Converts an external Inertia location visit into an Axum response.
    ///
    /// Inertia requests receive `409 Conflict` with `X-Inertia-Location`,
    /// or `X-Inertia-Redirect` for fragment destinations.
    /// Direct browser requests fall back to a method-aware redirect.
    pub fn location(&self, location: Location) -> Result<Response, InertiaError> {
        if self.context.is_inertia() {
            conflict_response(location.url())
        } else if is_write_method(&self.method) {
            redirect_response(StatusCode::SEE_OTHER, location.url())
        } else {
            redirect_response(StatusCode::FOUND, location.url())
        }
    }

    /// Converts a method-aware redirect into an Axum response.
    pub fn redirect(&self, redirect: Redirect) -> Result<Response, InertiaError> {
        if is_write_method(&self.method) {
            redirect_response(StatusCode::SEE_OTHER, redirect.url())
        } else {
            redirect_response(StatusCode::FOUND, redirect.url())
        }
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

/// Tower layer that installs Inertia asset version handling for Axum apps.
///
/// The layer inserts the current version into request extensions so
/// [`InertiaRequest::render`] can include it in page objects. For Inertia `GET`
/// requests whose `X-Inertia-Version` is missing or stale, it returns
/// `409 Conflict` with `X-Inertia-Location` before the route handler runs.
#[derive(Clone)]
pub struct VersionLayer {
    source: VersionSource,
}

impl VersionLayer {
    /// Creates a layer with a static asset `version`.
    pub fn new<V: Into<Arc<str>>>(version: V) -> Self {
        Self {
            source: VersionSource::Static(version.into()),
        }
    }

    /// Creates a layer with a dynamic asset-version provider.
    ///
    /// Keep the provider fast and non-blocking. If the version is loaded from
    /// disk or a manifest, cache it in application state and read the cached
    /// value here.
    pub fn dynamic<F, V>(version_provider: F) -> Self
    where
        F: Fn() -> V + Send + Sync + 'static,
        V: Into<Arc<str>>,
    {
        Self {
            source: VersionSource::Dynamic(Arc::new(move || version_provider().into())),
        }
    }
}

impl<S> Layer<S> for VersionLayer {
    type Service = VersionService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        VersionService {
            inner,
            source: self.source.clone(),
        }
    }
}

/// Service produced by [`VersionLayer`].
#[derive(Clone)]
pub struct VersionService<S> {
    inner: S,
    source: VersionSource,
}

impl<S, B> Service<Request<B>> for VersionService<S>
where
    S: Service<Request<B>, Response = Response>,
{
    type Response = Response;
    type Error = S::Error;
    type Future = VersionFuture<S::Future, S::Error>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut request: Request<B>) -> Self::Future {
        let should_check =
            request.method() == Method::GET && header(request.headers(), X_INERTIA).is_some();

        if should_check {
            let version = self.source.resolve();
            let request_version = header(request.headers(), crate::X_INERTIA_VERSION);

            if request_version != Some(version.as_ref()) {
                let response = conflict_response(&original_uri_from_extensions(&request))
                    .unwrap_or_else(internal_error_response);

                return VersionFuture::Ready {
                    result: Some(Ok(response)),
                };
            }

            request
                .extensions_mut()
                .insert(InertiaVersion::new(version));
        } else {
            request
                .extensions_mut()
                .insert(InertiaVersionSource(self.source.clone()));
        }

        VersionFuture::Inner {
            future: self.inner.call(request),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        InertiaProps, ScrollProps, X_INERTIA_EXCEPT_ONCE_PROPS,
        X_INERTIA_INFINITE_SCROLL_MERGE_INTENT, X_INERTIA_PARTIAL_COMPONENT,
        X_INERTIA_PARTIAL_DATA, X_INERTIA_PARTIAL_EXCEPT, X_INERTIA_RESET, X_INERTIA_VERSION,
    };
    use ::axum::body::Body;
    use ::axum::http::header::CONTENT_TYPE;
    use ::axum::http::{Request, StatusCode};
    use ::axum::response::Html;
    use ::axum::routing::get;
    use ::axum::Extension;
    use ::axum::Router;
    use serde::Serialize;
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use tower::ServiceExt;

    #[derive(Serialize)]
    struct Props {
        n: i32,
        plans: Vec<&'static str>,
        stats: i32,
        notifications: Vec<&'static str>,
    }

    #[derive(Serialize)]
    struct TextProps {
        text: String,
    }

    async fn page(request: InertiaRequest) -> Result<Response, InertiaError> {
        request.render(
            Inertia::response(
                "foo",
                Props {
                    n: 42,
                    plans: vec!["basic"],
                    stats: 7,
                    notifications: vec!["welcome"],
                },
            )
            .once("plans")
            .defer("stats"),
            |context| {
                Html(format!(
                    "<!doctype html><script data-page=\"app\" type=\"application/json\">{}</script>",
                    context.data_page()
                ))
            },
        )
    }

    async fn custom_url(request: InertiaRequest) -> Result<Response, InertiaError> {
        request.render(
            Inertia::response(
                "custom",
                Props {
                    n: 42,
                    plans: vec!["basic"],
                    stats: 7,
                    notifications: vec!["welcome"],
                },
            )
            .with_url("/custom-url"),
            |context| Html(context.data_page().to_owned()),
        )
    }

    async fn route_auth(request: InertiaRequest) -> Result<Response, InertiaError> {
        request.render(
            Inertia::response(
                "route-auth",
                json!({
                    "auth": {
                        "user": {
                            "name": "Route"
                        }
                    }
                }),
            ),
            |context| Html(context.data_page().to_owned()),
        )
    }

    async fn builder_page(request: InertiaRequest) -> Result<Response, InertiaError> {
        request.render(
            Inertia::page("builder")
                .merge("notifications")
                .props(Props {
                    n: 42,
                    plans: vec!["basic"],
                    stats: 7,
                    notifications: vec!["welcome"],
                }),
            |context| Html(context.data_page().to_owned()),
        )
    }

    async fn lazy_page(request: InertiaRequest) -> Result<Response, InertiaError> {
        request.render(
            Inertia::response(
                "lazy",
                InertiaProps::new()
                    .value("users", json!(["Ada", "Grace"]))
                    .defer("stats", || 7)
                    .optional("audit", || json!(["created"])),
            ),
            |context| Html(context.data_page().to_owned()),
        )
    }

    async fn empty(request: InertiaRequest) -> Result<Response, InertiaError> {
        request.render(Inertia::response("empty", ()), |context| {
            Html(context.data_page().to_owned())
        })
    }

    async fn scrolling(request: InertiaRequest) -> Result<Response, InertiaError> {
        request.render(
            Inertia::page("scrolling")
                .scroll("posts", ScrollProps::new("page", 1).next_page(2))
                .props(json!({ "posts": { "data": [1, 2] } })),
            |context| Html(context.data_page().to_owned()),
        )
    }

    async fn history_page(request: InertiaRequest) -> Result<Response, InertiaError> {
        request.render(
            Inertia::response("history", ())
                .encrypt_history()
                .clear_history()
                .preserve_fragment(),
            |context| Html(context.data_page().to_owned()),
        )
    }

    async fn unsafe_page(request: InertiaRequest) -> Result<Response, InertiaError> {
        request.render(
            Inertia::response(
                "unsafe",
                TextProps {
                    text: "</script><script>alert(1)</script>&\u{2028}\u{2029}".into(),
                },
            ),
            |context| {
                Html(format!(
                    "<!doctype html><script data-page=\"app\" type=\"application/json\">{}</script>",
                    context.data_page()
                ))
            },
        )
    }

    async fn external(request: InertiaRequest) -> Result<Response, InertiaError> {
        request.location(Inertia::location("https://example.com/outside"))
    }

    async fn relative_location(request: InertiaRequest) -> Result<Response, InertiaError> {
        request.location(Inertia::location("../outside?from=axum#fragment"))
    }

    async fn bad_location(request: InertiaRequest) -> Result<Response, InertiaError> {
        request.location(Inertia::location("bad location"))
    }

    async fn redirect(request: InertiaRequest) -> Result<Response, InertiaError> {
        request.redirect(Inertia::redirect("/target"))
    }

    async fn relative_redirect(request: InertiaRequest) -> Result<Response, InertiaError> {
        request.redirect(Inertia::redirect("?next=target#fragment"))
    }

    async fn bad_redirect(request: InertiaRequest) -> Result<Response, InertiaError> {
        request.redirect(Inertia::redirect("bad location"))
    }

    fn app() -> Router {
        Router::new()
            .route("/foo", get(page).post(page))
            .route("/custom-url", get(custom_url))
            .route("/builder", get(builder_page))
            .route("/route-auth", get(route_auth))
            .route("/lazy", get(lazy_page))
            .route("/empty", get(empty))
            .route("/scrolling", get(scrolling))
            .route("/history", get(history_page))
            .route("/unsafe", get(unsafe_page))
            .route("/external", get(external).post(external))
            .route("/relative-external", get(relative_location))
            .route("/bad-location", get(bad_location))
            .route(
                "/go",
                get(redirect)
                    .post(redirect)
                    .put(redirect)
                    .patch(redirect)
                    .delete(redirect),
            )
            .route("/relative-go", get(relative_redirect))
            .route("/bad-go", get(bad_redirect))
            .layer(VersionLayer::new("1"))
    }

    fn app_with_shared_props() -> Router {
        let shared_props = SharedProps::new()
            .value("appName", "Demo")
            .value("n", 99)
            .value("auth.user", json!({ "name": "Ada" }))
            .value("csrfToken", "token-shared");

        app_with_shared_props_registry(shared_props)
    }

    fn app_with_shared_props_registry(shared_props: SharedProps) -> Router {
        app().layer(Extension(shared_props))
    }

    fn app_without_layer() -> Router {
        Router::new().route("/foo", get(page))
    }

    fn dynamic_app(version: Arc<AtomicUsize>) -> Router {
        Router::new()
            .route("/foo", get(page))
            .layer(VersionLayer::dynamic(move || {
                format!("dynamic-{}", version.load(Ordering::SeqCst))
            }))
    }

    fn nested_app() -> Router {
        Router::new().nest(
            "/nested",
            Router::new()
                .route("/foo", get(page))
                .layer(VersionLayer::new("1")),
        )
    }

    async fn body_json(response: Response) -> serde_json::Value {
        let bytes = ::axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();

        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn html_response_includes_escaped_page_and_version() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/foo?bar=baz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(VARY)
                .and_then(|value| value.to_str().ok()),
            Some(X_INERTIA)
        );

        let body = ::axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body = std::str::from_utf8(&body).unwrap();

        assert!(body.contains("\"url\":\"/foo?bar=baz\""));
        assert!(body.contains("\"version\":\"1\""));
    }

    #[tokio::test]
    async fn html_response_escapes_json_for_script_context() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/unsafe")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = ::axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body = std::str::from_utf8(&body).unwrap();

        assert!(!body.contains("</script><script>"));
        assert!(body.contains("\\u003C/script\\u003E"));
        assert!(body.contains("\\u0026"));
        assert!(body.contains("\\u2028\\u2029"));
    }

    #[tokio::test]
    async fn inertia_json_response_includes_headers_url_and_version() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/foo?bar=baz")
                    .header(X_INERTIA, "true")
                    .header(X_INERTIA_VERSION, "1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("application/json")
        );
        assert_eq!(
            response
                .headers()
                .get(X_INERTIA)
                .and_then(|value| value.to_str().ok()),
            Some("true")
        );
        assert_eq!(
            response
                .headers()
                .get(VARY)
                .and_then(|value| value.to_str().ok()),
            Some(X_INERTIA)
        );

        let page = body_json(response).await;

        assert_eq!(page["component"], "foo");
        assert_eq!(page["url"], "/foo?bar=baz");
        assert_eq!(page["version"], "1");
        assert_eq!(page["props"]["n"], 42);
    }

    #[tokio::test]
    async fn nested_routes_use_original_uri_for_page_urls() {
        let response = nested_app()
            .oneshot(
                Request::builder()
                    .uri("/nested/foo?bar=baz")
                    .header(X_INERTIA, "true")
                    .header(X_INERTIA_VERSION, "1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let page = body_json(response).await;

        assert_eq!(page["url"], "/nested/foo?bar=baz");
    }

    #[tokio::test]
    async fn nested_absolute_form_requests_use_local_page_urls() {
        let response = nested_app()
            .oneshot(
                Request::builder()
                    .uri("http://example.test/nested/foo?bar=baz")
                    .header(X_INERTIA, "true")
                    .header(X_INERTIA_VERSION, "1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let page = body_json(response).await;

        assert_eq!(page["url"], "/nested/foo?bar=baz");
    }

    #[tokio::test]
    async fn render_respects_explicit_url_override() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/custom-url?ignored=true")
                    .header(X_INERTIA, "true")
                    .header(X_INERTIA_VERSION, "1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let page = body_json(response).await;

        assert_eq!(page["url"], "/custom-url");
    }

    #[tokio::test]
    async fn render_supports_advanced_builder_pages() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/builder")
                    .header(X_INERTIA, "true")
                    .header(X_INERTIA_VERSION, "1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let page = body_json(response).await;

        assert_eq!(page["component"], "builder");
        assert_eq!(page["mergeProps"], json!(["notifications"]));
    }

    #[tokio::test]
    async fn render_supports_lazy_props() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/lazy")
                    .header(X_INERTIA, "true")
                    .header(X_INERTIA_VERSION, "1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let page = body_json(response).await;

        assert_eq!(page["props"]["users"], json!(["Ada", "Grace"]));
        assert!(page["props"].get("stats").is_none());
        assert!(page["props"].get("audit").is_none());
        assert_eq!(page["deferredProps"], json!({ "default": ["stats"] }));

        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/lazy")
                    .header(X_INERTIA, "true")
                    .header(X_INERTIA_VERSION, "1")
                    .header(X_INERTIA_PARTIAL_COMPONENT, "lazy")
                    .header(X_INERTIA_PARTIAL_DATA, "stats,audit")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let page = body_json(response).await;

        assert_eq!(page["props"]["stats"], 7);
        assert_eq!(page["props"]["audit"], json!(["created"]));
        assert!(page["props"].get("users").is_none());
        assert!(page.get("deferredProps").is_none());
    }

    #[tokio::test]
    async fn shared_props_are_merged_into_html_responses() {
        let response = app_with_shared_props()
            .oneshot(Request::builder().uri("/foo").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = ::axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body = std::str::from_utf8(&body).unwrap();
        let data_page = body
            .split("<script data-page=\"app\" type=\"application/json\">")
            .nth(1)
            .and_then(|tail| tail.split("</script>").next())
            .unwrap();
        let page: serde_json::Value = serde_json::from_str(data_page).unwrap();

        assert_eq!(page["props"]["appName"], "Demo");
        assert_eq!(page["props"]["auth"]["user"]["name"], "Ada");
        assert_eq!(page["props"]["csrfToken"], "token-shared");
        assert_eq!(page["props"]["n"], 42);
        assert_eq!(page["sharedProps"], json!(["appName", "auth", "csrfToken"]));
    }

    #[tokio::test]
    async fn shared_props_are_merged_into_json_responses() {
        let response = app_with_shared_props()
            .oneshot(
                Request::builder()
                    .uri("/foo")
                    .header(X_INERTIA, "true")
                    .header(X_INERTIA_VERSION, "1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let page = body_json(response).await;

        assert_eq!(page["props"]["appName"], "Demo");
        assert_eq!(page["props"]["auth"]["user"]["name"], "Ada");
        assert_eq!(page["props"]["csrfToken"], "token-shared");
        assert_eq!(page["props"]["n"], 42);
        assert_eq!(page["sharedProps"], json!(["appName", "auth", "csrfToken"]));
    }

    #[tokio::test]
    async fn partial_reloads_include_shared_props_but_preserve_route_owned_roots() {
        let response = app_with_shared_props()
            .oneshot(
                Request::builder()
                    .uri("/foo")
                    .header(X_INERTIA, "true")
                    .header(X_INERTIA_VERSION, "1")
                    .header(X_INERTIA_PARTIAL_COMPONENT, "foo")
                    .header(X_INERTIA_PARTIAL_DATA, "stats")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let page = body_json(response).await;

        assert_eq!(page["props"]["stats"], 7);
        assert!(page["props"].get("n").is_none());
        assert_eq!(page["props"]["appName"], "Demo");
        assert_eq!(page["props"]["auth"]["user"]["name"], "Ada");
        assert_eq!(page["props"]["csrfToken"], "token-shared");
        assert_eq!(page["sharedProps"], json!(["appName", "auth", "csrfToken"]));
    }

    #[tokio::test]
    async fn shared_props_promote_non_object_props_to_an_object() {
        let response = app_with_shared_props_registry(SharedProps::new().value("appName", "Demo"))
            .oneshot(
                Request::builder()
                    .uri("/empty")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body = ::axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let page: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(page["props"]["errors"], json!({}));
        assert_eq!(page["props"]["appName"], "Demo");
        assert_eq!(page["sharedProps"], json!(["appName"]));
    }

    #[tokio::test]
    async fn partial_except_takes_precedence_over_partial_data() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/foo")
                    .header(X_INERTIA, "true")
                    .header(X_INERTIA_VERSION, "1")
                    .header(X_INERTIA_PARTIAL_COMPONENT, "foo")
                    .header(X_INERTIA_PARTIAL_DATA, "n,stats")
                    .header(X_INERTIA_PARTIAL_EXCEPT, "stats")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let page = body_json(response).await;

        assert_eq!(page["props"]["n"], 42);
        assert!(page["props"].get("stats").is_none());
    }

    #[tokio::test]
    async fn partial_reload_component_mismatch_ignores_partial_filtering() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/foo")
                    .header(X_INERTIA, "true")
                    .header(X_INERTIA_VERSION, "1")
                    .header(X_INERTIA_PARTIAL_COMPONENT, "other")
                    .header(X_INERTIA_PARTIAL_DATA, "n")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let page = body_json(response).await;

        assert_eq!(page["props"]["n"], 42);
        assert!(page["props"].get("stats").is_none());
        assert_eq!(page["props"]["notifications"], json!(["welcome"]));
    }

    #[tokio::test]
    async fn reset_omits_merge_and_scroll_metadata_for_reset_props() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/scrolling")
                    .header(X_INERTIA, "true")
                    .header(X_INERTIA_VERSION, "1")
                    .header(X_INERTIA_PARTIAL_COMPONENT, "scrolling")
                    .header(X_INERTIA_PARTIAL_DATA, "posts")
                    .header(X_INERTIA_RESET, "posts")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let page = body_json(response).await;

        assert!(page.get("mergeProps").is_none());
        assert!(page.get("prependProps").is_none());
        assert!(page.get("scrollProps").is_none());
    }

    #[tokio::test]
    async fn infinite_scroll_prepend_intent_sets_prepend_metadata() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/scrolling")
                    .header(X_INERTIA, "true")
                    .header(X_INERTIA_VERSION, "1")
                    .header(X_INERTIA_PARTIAL_COMPONENT, "scrolling")
                    .header(X_INERTIA_PARTIAL_DATA, "posts")
                    .header(X_INERTIA_INFINITE_SCROLL_MERGE_INTENT, "prepend")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let page = body_json(response).await;

        assert_eq!(page["prependProps"], json!(["posts.data"]));
        assert_eq!(page["scrollProps"]["posts"]["nextPage"], 2);
    }

    #[tokio::test]
    async fn matching_version_preserves_not_found_responses() {
        let response = Router::new()
            .layer(VersionLayer::new("1"))
            .oneshot(
                Request::builder()
                    .uri("/missing")
                    .header(X_INERTIA, "true")
                    .header(X_INERTIA_VERSION, "1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn history_flags_are_preserved_through_axum_rendering() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/history")
                    .header(X_INERTIA, "true")
                    .header(X_INERTIA_VERSION, "1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let page = body_json(response).await;

        assert_eq!(page["encryptHistory"], true);
        assert_eq!(page["clearHistory"], true);
        assert_eq!(page["preserveFragment"], true);
    }

    #[tokio::test]
    async fn skipped_colliding_shared_props_are_not_resolved() {
        let calls = Arc::new(AtomicUsize::new(0));
        let shared_props = SharedProps::new()
            .value("appName", "Demo")
            .prop("auth.user", {
                let calls = Arc::clone(&calls);
                move |_request| {
                    calls.fetch_add(1, Ordering::SeqCst);
                    json!({ "name": "Shared" })
                }
            });
        let response = app_with_shared_props_registry(shared_props)
            .oneshot(
                Request::builder()
                    .uri("/route-auth")
                    .header(X_INERTIA, "true")
                    .header(X_INERTIA_VERSION, "1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let page = body_json(response).await;

        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(page["props"]["auth"]["user"]["name"], "Route");
        assert_eq!(page["props"]["appName"], "Demo");
        assert_eq!(page["sharedProps"], json!(["appName"]));
    }

    #[tokio::test]
    async fn shared_props_see_raw_non_get_context() {
        let shared_props = SharedProps::new().prop("partialComponent", |request| {
            request
                .context()
                .partial_component()
                .unwrap_or("none")
                .to_owned()
        });
        let response = app_with_shared_props_registry(shared_props)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/foo")
                    .header(X_INERTIA, "true")
                    .header(X_INERTIA_PARTIAL_COMPONENT, "foo")
                    .header(X_INERTIA_PARTIAL_DATA, "stats")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let page = body_json(response).await;

        assert_eq!(page["props"]["partialComponent"], "foo");
        assert_eq!(page["props"]["n"], 42);
        assert_eq!(page["props"]["notifications"], json!(["welcome"]));
        assert!(page["props"].get("stats").is_none());
    }

    #[tokio::test]
    async fn optional_shared_props_can_skip_missing_values() {
        let shared_props = SharedProps::new()
            .value("appName", "Demo")
            .prop_optional("csrfToken", |_request| Option::<String>::None);
        let response = app_with_shared_props_registry(shared_props)
            .oneshot(
                Request::builder()
                    .uri("/foo")
                    .header(X_INERTIA, "true")
                    .header(X_INERTIA_VERSION, "1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let page = body_json(response).await;

        assert_eq!(page["props"]["appName"], "Demo");
        assert!(page["props"].get("csrfToken").is_none());
        assert_eq!(page["sharedProps"], json!(["appName"]));
    }

    #[tokio::test]
    async fn dotted_shared_props_merge_with_each_other() {
        let shared_props = SharedProps::new()
            .value("auth.user", json!({ "name": "Ada" }))
            .value("auth.csrf", "token-shared");
        let response = app_with_shared_props_registry(shared_props)
            .oneshot(
                Request::builder()
                    .uri("/lazy")
                    .header(X_INERTIA, "true")
                    .header(X_INERTIA_VERSION, "1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let page = body_json(response).await;

        assert_eq!(page["props"]["auth"]["user"]["name"], "Ada");
        assert_eq!(page["props"]["auth"]["csrf"], "token-shared");
        assert_eq!(page["sharedProps"], json!(["auth"]));
    }

    #[tokio::test]
    async fn empty_shared_props_are_a_noop_for_non_object_props() {
        let request = request_context(&HeaderMap::new());
        let request = InertiaRequest {
            context: request,
            method: Method::GET,
            shared_props: Some(SharedProps::new()),
            uri: "/empty".into(),
            version: None,
        };
        let response = request
            .render(Inertia::response("empty", ()), |context| {
                Html(context.data_page().to_owned())
            })
            .unwrap();
        let body = ::axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let page: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(page["props"], serde_json::Value::Null);
        assert!(page.get("sharedProps").is_none());
    }

    struct CloneProbe(Arc<AtomicUsize>);

    impl Clone for CloneProbe {
        fn clone(&self) -> Self {
            self.0.fetch_add(1, Ordering::SeqCst);
            Self(Arc::clone(&self.0))
        }
    }

    #[tokio::test]
    async fn inertia_extraction_does_not_clone_all_extensions() {
        let clones = Arc::new(AtomicUsize::new(0));
        let request = Request::builder()
            .uri("/")
            .extension(CloneProbe(Arc::clone(&clones)))
            .body(())
            .unwrap();
        let (mut parts, _) = request.into_parts();

        InertiaRequest::from_request_parts(&mut parts, &())
            .await
            .unwrap();

        assert_eq!(clones.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn shared_prop_serialization_errors_become_internal_server_errors() {
        let shared_props = SharedProps::new().prop("bad", |_request| {
            let mut value = BTreeMap::new();
            value.insert((1, 2), 3);
            value
        });
        let response = app_with_shared_props_registry(shared_props)
            .oneshot(
                Request::builder()
                    .uri("/foo")
                    .header(X_INERTIA, "true")
                    .header(X_INERTIA_VERSION, "1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn shared_dotted_props_do_not_replace_route_owned_roots() {
        let response = app_with_shared_props()
            .oneshot(
                Request::builder()
                    .uri("/route-auth")
                    .header(X_INERTIA, "true")
                    .header(X_INERTIA_VERSION, "1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let page = body_json(response).await;

        assert_eq!(page["props"]["auth"]["user"]["name"], "Route");
        assert_eq!(page["props"]["appName"], "Demo");
        assert_eq!(page["props"]["n"], 99);
        assert_eq!(page["props"]["csrfToken"], "token-shared");
        assert_eq!(page["sharedProps"], json!(["appName", "n", "csrfToken"]));
    }

    #[tokio::test]
    async fn response_without_version_layer_omits_asset_version() {
        let response = app_without_layer()
            .oneshot(
                Request::builder()
                    .uri("/foo")
                    .header(X_INERTIA, "true")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let page = body_json(response).await;

        assert!(page.get("version").is_none());
    }

    #[tokio::test]
    async fn dynamic_version_is_resolved_for_each_page_response() {
        let version = Arc::new(AtomicUsize::new(1));
        let app = dynamic_app(version.clone());

        version.store(2, Ordering::SeqCst);
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/foo")
                    .header(X_INERTIA, "true")
                    .header(X_INERTIA_VERSION, "dynamic-2")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let page = body_json(response).await;

        assert_eq!(page["version"], "dynamic-2");

        version.store(3, Ordering::SeqCst);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/foo")
                    .header(X_INERTIA, "true")
                    .header(X_INERTIA_VERSION, "dynamic-3")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let page = body_json(response).await;

        assert_eq!(page["version"], "dynamic-3");
    }

    #[tokio::test]
    async fn stale_inertia_version_conflicts_before_handler_runs() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/foo?bar=baz")
                    .header(X_INERTIA, "true")
                    .header(X_INERTIA_VERSION, "stale")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response
                .headers()
                .get(X_INERTIA_LOCATION)
                .and_then(|value| value.to_str().ok()),
            Some("/foo?bar=baz")
        );
    }

    #[tokio::test]
    async fn nested_stale_version_conflicts_use_original_uri() {
        let response = nested_app()
            .oneshot(
                Request::builder()
                    .uri("/nested/foo?bar=baz")
                    .header(X_INERTIA, "true")
                    .header(X_INERTIA_VERSION, "stale")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response
                .headers()
                .get(X_INERTIA_LOCATION)
                .and_then(|value| value.to_str().ok()),
            Some("/nested/foo?bar=baz")
        );
    }

    #[tokio::test]
    async fn nested_absolute_form_conflicts_use_local_location() {
        let response = nested_app()
            .oneshot(
                Request::builder()
                    .uri("http://example.test/nested/foo?bar=baz")
                    .header(X_INERTIA, "true")
                    .header(X_INERTIA_VERSION, "stale")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response
                .headers()
                .get(X_INERTIA_LOCATION)
                .and_then(|value| value.to_str().ok()),
            Some("/nested/foo?bar=baz")
        );
    }

    #[tokio::test]
    async fn missing_inertia_version_conflicts_before_handler_runs() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/foo")
                    .header(X_INERTIA, "true")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response
                .headers()
                .get(X_INERTIA_LOCATION)
                .and_then(|value| value.to_str().ok()),
            Some("/foo")
        );
    }

    #[tokio::test]
    async fn post_response_ignores_partial_reload_but_preserves_once_exclusions() {
        let response = app()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/foo")
                    .header(X_INERTIA, "true")
                    .header(X_INERTIA_VERSION, "stale")
                    .header(X_INERTIA_PARTIAL_COMPONENT, "foo")
                    .header(X_INERTIA_PARTIAL_DATA, "n")
                    .header(X_INERTIA_EXCEPT_ONCE_PROPS, "plans")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let page = body_json(response).await;

        assert_eq!(
            page["props"],
            json!({
                "errors": {},
                "n": 42,
                "notifications": ["welcome"]
            })
        );
        assert_eq!(
            page["onceProps"]["plans"],
            json!({ "prop": "plans", "expiresAt": null })
        );
    }

    #[tokio::test]
    async fn external_location_uses_conflict_for_inertia_requests() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/external")
                    .header(X_INERTIA, "true")
                    .header(X_INERTIA_VERSION, "1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response
                .headers()
                .get(X_INERTIA_LOCATION)
                .and_then(|value| value.to_str().ok()),
            Some("https://example.com/outside")
        );
    }

    #[tokio::test]
    async fn external_location_falls_back_to_browser_redirects() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/external")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FOUND);
        assert_eq!(
            response
                .headers()
                .get(LOCATION)
                .and_then(|value| value.to_str().ok()),
            Some("https://example.com/outside")
        );

        let response = app()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/external")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            response
                .headers()
                .get(LOCATION)
                .and_then(|value| value.to_str().ok()),
            Some("https://example.com/outside")
        );
    }

    #[tokio::test]
    async fn relative_location_references_are_supported() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/relative-external")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FOUND);
        assert_eq!(
            response
                .headers()
                .get(LOCATION)
                .and_then(|value| value.to_str().ok()),
            Some("../outside?from=axum#fragment")
        );
    }

    #[tokio::test]
    async fn fragment_location_uses_inertia_redirect_for_inertia_requests() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/relative-external")
                    .header(X_INERTIA, "true")
                    .header(X_INERTIA_VERSION, "1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response
                .headers()
                .get(X_INERTIA_REDIRECT)
                .and_then(|value| value.to_str().ok()),
            Some("../outside?from=axum#fragment")
        );
        assert_eq!(response.headers().get(X_INERTIA_LOCATION), None);
    }

    #[tokio::test]
    async fn invalid_location_returns_internal_server_error() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/bad-location")
                    .header(X_INERTIA, "true")
                    .header(X_INERTIA_VERSION, "1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn invalid_redirect_returns_internal_server_error() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/bad-go")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn all_write_redirect_methods_use_see_other_status() {
        for method in [Method::POST, Method::PUT, Method::PATCH, Method::DELETE] {
            let response = app()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri("/go")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::SEE_OTHER);
            assert_eq!(
                response
                    .headers()
                    .get(LOCATION)
                    .and_then(|value| value.to_str().ok()),
                Some("/target")
            );
        }
    }

    #[tokio::test]
    async fn get_redirects_use_found_status() {
        let response = app()
            .oneshot(Request::builder().uri("/go").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FOUND);
        assert_eq!(
            response
                .headers()
                .get(LOCATION)
                .and_then(|value| value.to_str().ok()),
            Some("/target")
        );
    }

    #[tokio::test]
    async fn relative_redirect_references_are_supported() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/relative-go")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FOUND);
        assert_eq!(
            response
                .headers()
                .get(LOCATION)
                .and_then(|value| value.to_str().ok()),
            Some("?next=target#fragment")
        );
    }
}
