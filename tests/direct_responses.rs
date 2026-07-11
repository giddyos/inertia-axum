use axum::{
    body::{to_bytes, Body},
    http::{
        header::{CONTENT_TYPE, LOCATION},
        Method, Request, StatusCode,
    },
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use inertia_axum::{
    page, DynamicPage, InertiaApp, Location, PendingResponseHandle, Redirect, RootContext,
    RootView, RouterInertiaExt, Visit, X_INERTIA, X_INERTIA_LOCATION, X_INERTIA_REDIRECT,
    X_INERTIA_VERSION,
};
use serde_json::{json, Value};
use std::{
    convert::Infallible,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use tower::ServiceExt;

const VERSION: &str = "phase-1";

async fn body(response: Response) -> String {
    String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap()
}

fn app() -> Router {
    async fn home() -> DynamicPage {
        page!("Home", { message: "Hello" })
    }
    async fn health() -> &'static str {
        "ok"
    }
    Router::new()
        .route("/", get(home))
        .route("/health", get(health))
        .inertia(InertiaApp::default_root().version(VERSION).build().unwrap())
}

fn request(method: Method, uri: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

fn inertia_request(method: Method, uri: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(X_INERTIA, "true")
        .header(X_INERTIA_VERSION, VERSION)
        .body(Body::empty())
        .unwrap()
}

#[tokio::test]
async fn direct_page_is_finalized_as_inertia_json() {
    let response = app()
        .oneshot(inertia_request(Method::GET, "/?tab=all"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers().get(X_INERTIA).unwrap(), "true");
    assert_eq!(response.headers().get("vary").unwrap(), X_INERTIA);
    let page: Value = serde_json::from_str(&body(response).await).unwrap();
    assert_eq!(page["component"], "Home");
    assert_eq!(page["props"], json!({"errors": {}, "message": "Hello"}));
    assert_eq!(page["url"], "/?tab=all");
    assert_eq!(page["version"], VERSION);
}

#[test]
fn page_macro_accepts_mixed_shorthand_and_explicit_props() {
    let message = "Hello";
    let response = page!("Home", { message, count: 2 }).into_response();
    assert!(response
        .extensions()
        .get::<PendingResponseHandle>()
        .is_some());
}

#[tokio::test]
async fn initial_page_uses_safe_root_and_script_safe_json() {
    async fn unsafe_page() -> DynamicPage {
        page!("Unsafe", { text: "</script>&\u{2028}" })
    }
    let app = Router::new()
        .route("/", get(unsafe_page))
        .inertia(InertiaApp::default_root().build().unwrap());
    let response = app.oneshot(request(Method::GET, "/")).await.unwrap();
    assert!(response.headers()[CONTENT_TYPE]
        .to_str()
        .unwrap()
        .starts_with("text/html"));
    let html = body(response).await;
    assert!(html.starts_with("<!doctype html>"));
    assert!(html.contains("\\u003C/script\\u003E\\u0026\\u2028"));
    assert!(!html.contains("</script>&"));
}

#[tokio::test]
async fn ordinary_axum_response_passes_through_unchanged() {
    let response = app()
        .oneshot(request(Method::GET, "/health"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().get("vary").is_none());
    assert_eq!(body(response).await, "ok");
}

#[tokio::test]
async fn missing_layer_is_actionable_and_handle_is_one_shot() {
    let mut response = page!("Home", { message: "Hello" }).into_response();
    let handle = response
        .extensions_mut()
        .remove::<PendingResponseHandle>()
        .unwrap();
    assert!(handle.clone().take().is_some());
    assert!(handle.take().is_none());
    let response = page!("Home", { message: "Hello" }).into_response();
    let text = body(response).await;
    assert!(text.contains("Inertia layer is not installed"));
    assert!(text.contains(".inertia(inertia)"));
}

#[derive(Debug)]
struct AppError;

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        page!("Errors/Server", { message: "failed" })
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .into_response()
    }
}

#[tokio::test]
async fn error_pages_use_the_same_finalizer() {
    async fn failure() -> Result<DynamicPage, AppError> {
        Err(AppError)
    }
    let app = Router::new()
        .route("/", get(failure))
        .inertia(InertiaApp::default_root().build().unwrap());
    let response = app
        .oneshot(inertia_request_without_version(Method::GET, "/"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let page: Value = serde_json::from_str(&body(response).await).unwrap();
    assert_eq!(page["component"], "Errors/Server");
}

fn inertia_request_without_version(method: Method, uri: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(X_INERTIA, "true")
        .body(Body::empty())
        .unwrap()
}

#[tokio::test]
async fn direct_redirects_and_locations_preserve_protocol_semantics() {
    async fn redirect() -> Redirect {
        Redirect::to("/next")
    }
    async fn back() -> Redirect {
        Redirect::back_or("/fallback")
    }
    async fn location() -> Location {
        Location::external("https://example.com/docs#api")
    }
    let app = Router::new()
        .route("/redirect", get(redirect).post(redirect))
        .route("/back", post(back))
        .route("/location", get(location))
        .inertia(InertiaApp::default_root().build().unwrap());

    let response = app
        .clone()
        .oneshot(request(Method::GET, "/redirect"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FOUND);
    assert_eq!(response.headers()[LOCATION], "/next");
    let response = app
        .clone()
        .oneshot(request(Method::POST, "/redirect"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/back")
                .header("referer", "/origin")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.headers()[LOCATION], "/origin");
    let response = app
        .oneshot(inertia_request_without_version(Method::GET, "/location"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert!(response.headers().get(X_INERTIA_LOCATION).is_none());
    assert_eq!(
        response.headers()[X_INERTIA_REDIRECT],
        "https://example.com/docs#api"
    );
}

#[tokio::test]
async fn advanced_visit_is_available_but_optional() {
    async fn inspect(visit: Visit) -> String {
        format!(
            "{}:{}:{}",
            visit.is_inertia(),
            visit.is_prefetch(),
            visit.version().unwrap_or("none")
        )
    }
    let app = Router::new()
        .route("/", get(inspect))
        .inertia(InertiaApp::default_root().version(VERSION).build().unwrap());
    let response = app
        .oneshot(inertia_request(Method::GET, "/"))
        .await
        .unwrap();
    assert_eq!(body(response).await, "true:false:phase-1");
}

#[derive(Clone)]
struct CustomRoot;

impl RootView for CustomRoot {
    type Error = Infallible;
    fn render(&self, context: RootContext<'_>) -> Result<String, Self::Error> {
        Ok(format!("<main>{}</main>", context.mount()))
    }
}

#[tokio::test]
async fn custom_root_is_application_wide() {
    async fn home() -> DynamicPage {
        page!("Home", { message: "Hello" })
    }
    let app = Router::new()
        .route("/", get(home))
        .inertia(InertiaApp::builder(CustomRoot).build().unwrap());
    assert!(body(app.oneshot(request(Method::GET, "/")).await.unwrap())
        .await
        .starts_with("<main>"));
}

#[tokio::test]
async fn stale_version_short_circuits_before_handler() {
    let calls = Arc::new(AtomicUsize::new(0));
    let handler_calls = calls.clone();
    let app = Router::new()
        .route(
            "/",
            get(move || {
                let calls = handler_calls.clone();
                async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    page!("Home", { value: 1 })
                }
            }),
        )
        .inertia(InertiaApp::default_root().version(VERSION).build().unwrap());
    let request = Request::builder()
        .uri("/")
        .header(X_INERTIA, "true")
        .header(X_INERTIA_VERSION, "stale")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}
