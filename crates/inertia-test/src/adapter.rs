//! Framework-neutral in-process adapter conformance contract.

use async_trait::async_trait;
#[cfg(feature = "axum")]
use axum::{
    Router,
    body::{Body, to_bytes},
    http::Request,
    response::Response,
};
use bytes::Bytes;
#[cfg(any(feature = "actix", feature = "rocket"))]
use futures_util::future::LocalBoxFuture;
use http::{HeaderMap, Method, StatusCode, Uri};
#[cfg(any(feature = "actix", feature = "rocket"))]
use std::{future::Future, rc::Rc};
#[cfg(feature = "axum")]
use tower::ServiceExt as _;

/// Framework-neutral request passed to an adapter test driver.
pub struct AdapterRequest {
    /// HTTP method.
    pub method: Method,
    /// Request URI.
    pub uri: Uri,
    /// Request headers, including repeated values.
    pub headers: HeaderMap,
    /// Buffered request body.
    pub body: Bytes,
}

impl AdapterRequest {
    /// Creates an empty request.
    pub fn new(method: Method, uri: impl AsRef<str>) -> Self {
        Self {
            method,
            uri: uri
                .as_ref()
                .parse()
                .expect("adapter test URI must be valid"),
            headers: HeaderMap::new(),
            body: Bytes::new(),
        }
    }

    /// Adds or replaces one request header.
    pub fn header(mut self, name: &'static str, value: &str) -> Self {
        self.headers.insert(
            name,
            value.parse().expect("adapter test header must be valid"),
        );
        self
    }

    /// Sets the request body.
    pub fn body(mut self, body: impl Into<Bytes>) -> Self {
        self.body = body.into();
        self
    }
}

/// Framework-neutral buffered response returned by an adapter test driver.
pub struct AdapterResponse {
    /// HTTP status.
    pub status: StatusCode,
    /// Response headers, including repeated values.
    pub headers: HeaderMap,
    /// Buffered response body.
    pub body: Bytes,
}

impl AdapterResponse {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).and_then(|value| value.to_str().ok())
    }

    fn page(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).expect("response must contain an Inertia JSON page")
    }
}

/// In-process HTTP driver implemented by every framework adapter.
#[async_trait(?Send)]
pub trait AdapterHarness {
    /// Executes one buffered framework-neutral request.
    async fn request(&self, request: AdapterRequest) -> AdapterResponse;
}

/// Axum implementation of the shared adapter harness.
#[cfg(feature = "axum")]
pub struct AxumHarness {
    installed: Router,
    uninstalled: Router,
}

#[cfg(feature = "axum")]
impl AxumHarness {
    /// Creates a harness with installed and deliberately uninstalled routers.
    pub fn new(installed: Router, uninstalled: Router) -> Self {
        Self {
            installed,
            uninstalled,
        }
    }
}

#[cfg(feature = "axum")]
#[async_trait(?Send)]
impl AdapterHarness for AxumHarness {
    async fn request(&self, request: AdapterRequest) -> AdapterResponse {
        let router = if request.uri.path() == "/missing" {
            &self.uninstalled
        } else {
            &self.installed
        };
        let AdapterRequest {
            method,
            uri,
            headers,
            body,
        } = request;
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .body(Body::from(body))
            .expect("adapter request must build");
        let (mut parts, body) = request.into_parts();
        parts.headers = headers;
        let response: Response = router
            .clone()
            .oneshot(Request::from_parts(parts, body))
            .await
            .expect("Axum adapter request must succeed");
        let status = response.status();
        let headers = response.headers().clone();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("Axum adapter response body must buffer");
        AdapterResponse {
            status,
            headers,
            body,
        }
    }
}

#[cfg(feature = "actix")]
type ActixRequest =
    Rc<dyn Fn(AdapterRequest) -> LocalBoxFuture<'static, AdapterResponse> + 'static>;

/// Actix Web implementation of the shared adapter harness.
#[cfg(feature = "actix")]
pub struct ActixHarness {
    request: ActixRequest,
}

#[cfg(feature = "actix")]
impl ActixHarness {
    /// Creates a harness from an Actix in-process request driver.
    pub fn new<F, Fut>(request: F) -> Self
    where
        F: Fn(AdapterRequest) -> Fut + 'static,
        Fut: Future<Output = AdapterResponse> + 'static,
    {
        Self {
            request: Rc::new(move |adapter_request| Box::pin(request(adapter_request))),
        }
    }
}

#[cfg(feature = "actix")]
#[async_trait(?Send)]
impl AdapterHarness for ActixHarness {
    async fn request(&self, request: AdapterRequest) -> AdapterResponse {
        (self.request)(request).await
    }
}

#[cfg(feature = "rocket")]
type RocketRequest =
    Rc<dyn Fn(AdapterRequest) -> LocalBoxFuture<'static, AdapterResponse> + 'static>;

/// Rocket implementation of the shared adapter harness.
#[cfg(feature = "rocket")]
pub struct RocketHarness {
    request: RocketRequest,
}

#[cfg(feature = "rocket")]
impl RocketHarness {
    /// Creates a harness from a Rocket in-process request driver.
    pub fn new<F, Fut>(request: F) -> Self
    where
        F: Fn(AdapterRequest) -> Fut + 'static,
        Fut: Future<Output = AdapterResponse> + 'static,
    {
        Self {
            request: Rc::new(move |adapter_request| Box::pin(request(adapter_request))),
        }
    }
}

#[cfg(feature = "rocket")]
#[async_trait(?Send)]
impl AdapterHarness for RocketHarness {
    async fn request(&self, request: AdapterRequest) -> AdapterResponse {
        (self.request)(request).await
    }
}

fn inertia(method: Method, uri: &str) -> AdapterRequest {
    AdapterRequest::new(method, uri)
        .header("x-inertia", "true")
        .header("x-inertia-version", "contract-v1")
}

fn partial(uri: &str, component: &str, data: &str) -> AdapterRequest {
    inertia(Method::GET, uri)
        .header("x-inertia-partial-component", component)
        .header("x-inertia-partial-data", data)
}

/// Runs the single behavioral contract shared by every adapter.
///
/// Harness fixtures use `/page`, `/redirect`, `/external`, `/form`,
/// `/flash`, `/ssr`, `/ssr-fallback`, `/health`, `/missing`, and `/build`.
pub async fn run_adapter_conformance(harness: &impl AdapterHarness) {
    let initial = harness
        .request(AdapterRequest::new(Method::GET, "/page"))
        .await;
    assert_eq!(initial.status, StatusCode::OK, "initial HTML visit");
    assert!(
        initial
            .header("content-type")
            .is_some_and(|value| value.starts_with("text/html")),
        "initial visit must be HTML"
    );
    assert!(
        String::from_utf8_lossy(&initial.body).contains("\"component\":\"Conformance\""),
        "initial HTML must embed the page"
    );

    let json = harness.request(inertia(Method::GET, "/page")).await;
    assert_eq!(json.status, StatusCode::OK, "Inertia JSON visit");
    let page = json.page();
    assert_eq!(page["component"], "Conformance");
    assert_eq!(page["props"]["ordinary"], "route");

    let partial_page = harness
        .request(partial("/page", "Conformance", "ordinary"))
        .await
        .page();
    assert_eq!(partial_page["props"]["ordinary"], "route", "partial reload");
    assert!(
        partial_page["props"].get("merged").is_none(),
        "matching partial reload must omit unselected props"
    );

    let mismatch = harness
        .request(partial("/page", "Other", "ordinary"))
        .await
        .page();
    assert!(
        mismatch["props"].get("merged").is_some(),
        "component mismatch must perform a full reload"
    );

    let version = harness
        .request(
            inertia(Method::GET, "/page")
                .header("x-inertia-version", "stale")
                .header("x-inertia-transient-id", "version"),
        )
        .await;
    assert_eq!(
        version.status,
        StatusCode::CONFLICT,
        "asset version mismatch"
    );
    assert_eq!(version.header("x-inertia-location"), Some("/page"));

    let external = harness.request(inertia(Method::GET, "/external")).await;
    assert_eq!(external.status, StatusCode::CONFLICT, "external redirect");
    assert_eq!(
        external.header("x-inertia-location"),
        Some("https://example.com/outside")
    );

    let redirect = harness.request(inertia(Method::POST, "/redirect")).await;
    assert_eq!(
        redirect.status,
        StatusCode::SEE_OTHER,
        "same-origin redirect"
    );
    assert_eq!(redirect.header("location"), Some("/page"));

    let validation = harness
        .request(
            inertia(Method::POST, "/form")
                .header("content-type", "application/json")
                .header("referer", "/page")
                .header("x-inertia-transient-id", "validation")
                .body(r#"{"title":""}"#),
        )
        .await;
    assert_eq!(
        validation.status,
        StatusCode::SEE_OTHER,
        "form validation redirect"
    );
    assert_eq!(validation.header("location"), Some("/page"));
    let validation_page = harness
        .request(inertia(Method::GET, "/page").header("x-inertia-transient-id", "validation"))
        .await
        .page();
    assert_eq!(
        validation_page["props"]["errors"]["title"], "required",
        "form validation errors"
    );

    assert_eq!(page["props"]["shared"], "adapter", "shared props");
    assert!(
        page["deferredProps"]["default"]
            .as_array()
            .is_some_and(|values| values.iter().any(|value| value == "deferred")),
        "deferred props"
    );

    let optional = harness
        .request(partial("/page", "Conformance", "optional"))
        .await
        .page();
    assert_eq!(optional["props"]["optional"], 2, "optional props");
    assert!(
        page["props"].get("optional").is_none(),
        "optional prop must not load eagerly"
    );

    assert!(
        page["mergeProps"]
            .as_array()
            .is_some_and(|values| values.iter().any(|value| value == "merged")),
        "merge props"
    );
    assert_eq!(page["onceProps"]["once"]["prop"], "once", "once props");
    assert!(page["scrollProps"].get("scroll").is_some(), "scroll props");

    let flash_redirect = harness
        .request(inertia(Method::POST, "/flash").header("x-inertia-transient-id", "flash"))
        .await;
    assert_eq!(flash_redirect.status, StatusCode::SEE_OTHER);
    let flashed = harness
        .request(inertia(Method::GET, "/page").header("x-inertia-transient-id", "flash"))
        .await
        .page();
    assert_eq!(flashed["flash"]["notice"], "saved", "transient flash");

    let ssr = harness
        .request(AdapterRequest::new(Method::GET, "/ssr"))
        .await;
    assert_eq!(ssr.status, StatusCode::OK, "SSR success");
    assert!(
        String::from_utf8_lossy(&ssr.body).contains("data-server-rendered=\"true\""),
        "SSR success must contain server markup"
    );

    let fallback = harness
        .request(AdapterRequest::new(Method::GET, "/ssr-fallback"))
        .await;
    assert_eq!(fallback.status, StatusCode::OK, "SSR fallback");
    assert!(
        !String::from_utf8_lossy(&fallback.body).contains("data-server-rendered=\"true\""),
        "SSR failure must fall back to CSR"
    );

    let missing = harness
        .request(AdapterRequest::new(Method::GET, "/missing"))
        .await;
    assert_eq!(
        missing.status,
        StatusCode::INTERNAL_SERVER_ERROR,
        "missing app installation"
    );

    let javascript = harness
        .request(AdapterRequest::new(Method::GET, "/build/assets/app.js"))
        .await;
    assert_eq!(javascript.status, StatusCode::OK, "embedded asset GET");
    assert_eq!(
        javascript.body,
        Bytes::from_static(b"console.log('adapter')")
    );
    let etag = javascript
        .header("etag")
        .expect("embedded asset must emit an ETag")
        .to_owned();

    let css = harness
        .request(AdapterRequest::new(Method::GET, "/build/assets/app.css"))
        .await;
    assert_eq!(css.status, StatusCode::OK, "embedded CSS GET");
    assert_eq!(css.body, Bytes::from_static(b"body{color:#123}"));

    let head = harness
        .request(AdapterRequest::new(Method::HEAD, "/build/assets/app.js"))
        .await;
    assert_eq!(head.status, StatusCode::OK, "embedded asset HEAD");
    assert!(head.body.is_empty(), "HEAD asset body must be empty");

    let not_modified = harness
        .request(
            AdapterRequest::new(Method::GET, "/build/assets/app.js").header("if-none-match", &etag),
        )
        .await;
    assert_eq!(
        not_modified.status,
        StatusCode::NOT_MODIFIED,
        "embedded asset ETag"
    );
    assert!(not_modified.body.is_empty(), "304 body must be empty");

    let absent = harness
        .request(AdapterRequest::new(Method::GET, "/build/assets/missing.js"))
        .await;
    assert_eq!(absent.status, StatusCode::NOT_FOUND, "missing asset");

    let traversal = harness
        .request(AdapterRequest::new(Method::GET, "/build/%2e%2e/secret.txt"))
        .await;
    assert_eq!(
        traversal.status,
        StatusCode::NOT_FOUND,
        "path traversal attempt"
    );

    let unsupported = harness
        .request(AdapterRequest::new(Method::POST, "/build/assets/app.js"))
        .await;
    assert_eq!(
        unsupported.status,
        StatusCode::METHOD_NOT_ALLOWED,
        "unsupported asset method"
    );

    let health = harness
        .request(AdapterRequest::new(Method::GET, "/health"))
        .await;
    assert_eq!(health.status, StatusCode::OK);
    assert_eq!(health.body, Bytes::from_static(b"healthy"));
}
