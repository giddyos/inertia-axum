#![cfg(feature = "ssr")]
#![allow(missing_docs)]

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
    routing::{get, post},
};
use inertia_axum::{DynamicPage, InertiaApp, RouterInertiaExt as _, Ssr, SsrRouteExt as _};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;
use tower::ServiceExt as _;

async fn fake(render_status: StatusCode, render_body: &'static str) -> (String, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let render_count = calls.clone();
    let vite_count = calls.clone();
    let app = Router::new()
        .route("/health", get(|| async { StatusCode::OK }))
        .route(
            "/render",
            post(move || {
                render_count.fetch_add(1, Ordering::SeqCst);
                async move { (render_status, render_body) }
            }),
        )
        .route(
            "/__inertia_ssr",
            post(move || {
                vite_count.fetch_add(1, Ordering::SeqCst);
                async move { (render_status, render_body) }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{address}"), calls)
}

async fn external(_base: &str, config: Ssr, route: axum::routing::MethodRouter) -> Router {
    let inertia = InertiaApp::default_root()
        .ssr(config)
        .start()
        .await
        .unwrap();
    Router::new().route("/", route).inertia(inertia)
}

async fn body(response: axum::response::Response) -> String {
    String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap()
}

fn page() -> DynamicPage {
    DynamicPage::new("Home").prop("message", "hello")
}

#[tokio::test]
async fn external_ssr_renders_unmarked_initial_get() {
    let (base, calls) = fake(StatusCode::OK, r#"{"head":["<title>SSR</title>"],"body":"<div id=\"app\" data-server-rendered=\"true\">SSR</div>"}"#).await;
    let app = external(&base, Ssr::external(&base), get(|| async { page() })).await;
    let html = body(
        app.oneshot(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap(),
    )
    .await;
    assert!(html.contains("data-server-rendered"));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn root_template_places_ssr_head_and_mount_once() {
    let (base, _) = fake(
        StatusCode::OK,
        r#"{"head":["<title>SSR</title>"],"body":"<div id=\"app\">SSR body</div>"}"#,
    )
    .await;
    let inertia = InertiaApp::default_root()
        .root_template_source("<html><head><!-- inertia:assets --><!-- inertia:head --></head><body data-template><!-- inertia:mount --></body></html>")
        .ssr(Ssr::external(&base)).start().await.unwrap();
    let html = body(
        Router::new()
            .route("/", get(|| async { page() }))
            .inertia(inertia)
            .oneshot(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(html.matches("<title>SSR</title>").count(), 1);
    assert_eq!(html.matches("<div id=\"app\">SSR body</div>").count(), 1);
    assert!(html.contains("<body data-template>"));
}

#[tokio::test]
async fn inertia_json_request_never_invokes_ssr() {
    let (base, calls) = fake(StatusCode::OK, r#"{"head":[],"body":"SSR"}"#).await;
    let app = external(&base, Ssr::external(&base), get(|| async { page() })).await;
    let response = app
        .oneshot(
            Request::get("/")
                .header("x-inertia", "true")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.headers().get("x-inertia").unwrap(), "true");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn non_get_request_never_invokes_ssr() {
    let (base, calls) = fake(StatusCode::OK, r#"{"head":[],"body":"SSR"}"#).await;
    let app = external(&base, Ssr::external(&base), post(|| async { page() })).await;
    let html = body(
        app.oneshot(Request::post("/").body(Body::empty()).unwrap())
            .await
            .unwrap(),
    )
    .await;
    assert!(html.contains("data-page=\"app\""));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn without_ssr_route_skips_rendering() {
    let (base, calls) = fake(StatusCode::OK, r#"{"head":[],"body":"SSR"}"#).await;
    let app = external(
        &base,
        Ssr::external(&base),
        get(|| async { page() }).without_ssr(),
    )
    .await;
    let html = body(
        app.oneshot(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap(),
    )
    .await;
    assert!(html.contains("<div id=\"app\"></div>"));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn with_ssr_route_renders_in_opt_in_mode() {
    let (base, calls) = fake(
        StatusCode::OK,
        r#"{"head":[],"body":"<div id=\"app\">SSR</div>"}"#,
    )
    .await;
    let app = external(
        &base,
        Ssr::external(&base).opt_in(),
        get(|| async { page() }).with_ssr(),
    )
    .await;
    assert!(
        body(
            app.oneshot(Request::get("/").body(Body::empty()).unwrap())
                .await
                .unwrap()
        )
        .await
        .contains("SSR")
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn ssr_when_selects_per_request() {
    let (base, calls) = fake(
        StatusCode::OK,
        r#"{"head":[],"body":"<div id=\"app\">SSR</div>"}"#,
    )
    .await;
    let app = external(
        &base,
        Ssr::external(&base),
        get(|| async { page() }).ssr_when(|context| context.headers().contains_key("x-ssr")),
    )
    .await;
    let csr = body(
        app.clone()
            .oneshot(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap(),
    )
    .await;
    let ssr = body(
        app.oneshot(
            Request::get("/")
                .header("x-ssr", "1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap(),
    )
    .await;
    assert!(csr.contains("data-page=\"app\""));
    assert!(ssr.contains(">SSR</div>"));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn opt_in_default_skips_unmarked_route() {
    let (base, calls) = fake(StatusCode::OK, r#"{"head":[],"body":"SSR"}"#).await;
    let app = external(
        &base,
        Ssr::external(&base).opt_in(),
        get(|| async { page() }),
    )
    .await;
    app.oneshot(Request::get("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn router_group_policy_is_respected() {
    let (base, calls) = fake(StatusCode::OK, r#"{"head":[],"body":"SSR"}"#).await;
    let inertia = InertiaApp::default_root()
        .ssr(Ssr::external(&base))
        .start()
        .await
        .unwrap();
    let group = Router::new()
        .route("/", get(|| async { page() }))
        .without_ssr();
    let app = Router::new().nest("/group", group).inertia(inertia);
    app.oneshot(Request::get("/group/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn ssr_head_body_and_status_are_preserved_without_double_wrap() {
    let (base, _) = fake(StatusCode::OK, r#"{"head":["<title>Server title</title>"],"body":"<div id=\"app\" data-server-rendered=\"true\">SSR</div>"}"#).await;
    let app = external(
        &base,
        Ssr::external(&base),
        get(|| async { page().status(StatusCode::CREATED) }),
    )
    .await;
    let response = app
        .oneshot(Request::get("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let html = body(response).await;
    assert!(html.contains("<title>Server title</title></head>"));
    assert_eq!(html.matches("id=\"app\"").count(), 1);
}

#[tokio::test]
async fn vite_development_ssr_needs_no_production_manifest_or_bundle() {
    let (base, calls) = fake(StatusCode::OK, "null").await;
    let missing_root = "target/definitely-missing-vite-production-output";
    assert!(
        !std::path::Path::new(missing_root)
            .join("dist/.vite/manifest.json")
            .exists()
    );
    assert!(
        !std::path::Path::new(missing_root)
            .join("missing-bundle.js")
            .exists()
    );
    let inertia = InertiaApp::vite(missing_root)
        .dev_server(&base)
        .ssr("missing-bundle.js")
        .start()
        .await
        .unwrap();
    let app = Router::new()
        .route("/", get(|| async { page() }))
        .inertia(inertia);
    let html = body(
        app.oneshot(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap(),
    )
    .await;
    assert!(html.contains("<div id=\"app\"></div>"));
    assert!(html.contains(&format!("{base}/@vite/client")));
    assert!(html.contains(&format!("{base}/src/main.ts")));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn failure_modes_fallback_or_return_internal_error() {
    let (base, _) = fake(StatusCode::INTERNAL_SERVER_ERROR, "broken").await;
    let fallback = external(&base, Ssr::external(&base), get(|| async { page() })).await;
    let strict = external(
        &base,
        Ssr::external(&base).strict(),
        get(|| async { page() }),
    )
    .await;
    let fallback_response = fallback
        .oneshot(Request::get("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let strict_response = strict
        .oneshot(Request::get("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(fallback_response.status(), StatusCode::OK);
    assert_eq!(strict_response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert!(!body(strict_response).await.contains("broken"));
}

#[tokio::test]
async fn timeout_and_malformed_or_large_response_fall_back_to_csr() {
    async fn run(body_value: &'static str, timeout: Duration, limit: usize) -> StatusCode {
        let app = Router::new()
            .route("/health", get(|| async { StatusCode::OK }))
            .route(
                "/render",
                post(move || async move {
                    if body_value == "slow" {
                        tokio::time::sleep(Duration::from_millis(75)).await;
                    }
                    if body_value == "slow" {
                        "null"
                    } else {
                        body_value
                    }
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let base = format!("http://{address}");
        let inertia = InertiaApp::default_root()
            .ssr(
                Ssr::external(&base)
                    .timeout(timeout)
                    .max_response_bytes(limit),
            )
            .start()
            .await
            .unwrap();
        Router::new()
            .route("/", get(|| async { page() }))
            .inertia(inertia)
            .oneshot(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status()
    }
    assert_eq!(
        run("slow", Duration::from_millis(5), 1024).await,
        StatusCode::OK
    );
    assert_eq!(
        run("not-json", Duration::from_secs(1), 1024).await,
        StatusCode::OK
    );
    assert_eq!(
        run("xxxxxxxxxxxxxxxxxxxxxxxx", Duration::from_secs(1), 8).await,
        StatusCode::OK
    );
}

#[tokio::test]
async fn strict_timeout_returns_error() {
    let app = Router::new()
        .route("/health", get(|| async { StatusCode::OK }))
        .route(
            "/render",
            post(|| async {
                tokio::time::sleep(Duration::from_millis(50)).await;
                "null"
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let base = format!("http://{address}");
    let app = external(
        &base,
        Ssr::external(&base)
            .timeout(Duration::from_millis(5))
            .strict(),
        get(|| async { page() }),
    )
    .await;
    assert_eq!(
        app.oneshot(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[tokio::test]
async fn successful_render_restores_ready_health_and_accessor_is_local() {
    use inertia_axum::{SsrBackendKind, SsrHealth};
    let (base, _) = fake(
        StatusCode::OK,
        r#"{"head":[],"body":"<div id=\"app\">SSR</div>"}"#,
    )
    .await;
    let inertia = InertiaApp::default_root()
        .ssr(Ssr::external(&base))
        .start()
        .await
        .unwrap();
    assert_eq!(
        inertia.ssr_health(),
        SsrHealth::Ready {
            backend: SsrBackendKind::External
        }
    );
    let app = Router::new()
        .route("/", get(|| async { page() }))
        .inertia(inertia.clone());
    app.oneshot(Request::get("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(
        inertia.ssr_health(),
        SsrHealth::Ready {
            backend: SsrBackendKind::External
        }
    );
}
