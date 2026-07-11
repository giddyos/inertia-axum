//! Axum integration test fixtures and response behavior tests.

use crate::axum::response_headers::request_context;
use crate::axum::*;
use crate::*;
use ::axum::extract::FromRequestParts;
use ::axum::http::header::LOCATION;
use ::axum::http::{HeaderMap, Method};
use ::axum::response::Response;

use crate::{
    InertiaProps, ScrollProps, X_INERTIA_EXCEPT_ONCE_PROPS, X_INERTIA_INFINITE_SCROLL_MERGE_INTENT,
    X_INERTIA_PARTIAL_COMPONENT, X_INERTIA_PARTIAL_DATA, X_INERTIA_PARTIAL_EXCEPT, X_INERTIA_RESET,
    X_INERTIA_VERSION,
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
        referer: None,
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
