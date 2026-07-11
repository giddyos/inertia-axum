#![cfg(feature = "askama")]
#![allow(missing_docs)]

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header::CONTENT_TYPE},
    routing::get,
};
use inertia_axum::{
    AskamaRoot, AskamaRootContext, DynamicPage, InertiaApp, RootContext, RootView,
    RouterInertiaExt as _,
    askama::{self, Template},
};
use std::convert::Infallible;
use tower::ServiceExt as _;

const MARKER_TEMPLATE: &str = "<html><head><!-- inertia:assets --><!-- inertia:head --></head><body data-root=\"marker\"><!-- inertia:mount --></body></html>";

#[derive(Template)]
#[template(
    source = "<!doctype html><html><head><meta name=\"root\" content=\"{{ label }}\">{{ inertia.assets|safe }}{{ inertia.head|safe }}</head><body data-root=\"askama\">{{ inertia.mount|safe }}</body></html>",
    ext = "html",
    askama = askama
)]
struct TestTemplate<'a> {
    inertia: AskamaRootContext<'a>,
    label: &'a str,
}

#[derive(Clone)]
struct TestRoot(&'static str);

impl AskamaRoot for TestRoot {
    type Template<'a> = TestTemplate<'a>;

    fn template<'a>(&'a self, inertia: AskamaRootContext<'a>) -> Self::Template<'a> {
        TestTemplate {
            inertia,
            label: self.0,
        }
    }
}

#[derive(Clone)]
struct CustomRoot;

impl RootView for CustomRoot {
    type Error = Infallible;

    fn render(&self, context: RootContext<'_>) -> Result<String, Self::Error> {
        Ok(format!("<custom>{}</custom>", context.mount()))
    }
}

async fn page() -> DynamicPage {
    DynamicPage::new("Home").prop("unsafe", "</script><script>alert(1)</script>")
}

fn router(inertia: InertiaApp) -> Router {
    Router::new().route("/", get(page)).inertia(inertia)
}

async fn response(inertia: InertiaApp, is_inertia: bool) -> axum::response::Response {
    let mut request = Request::get("/");
    if is_inertia {
        request = request.header("x-inertia", "true");
    }
    router(inertia)
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap()
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

#[tokio::test]
async fn initial_request_uses_askama_root_and_keeps_page_script_safe() {
    let inertia = InertiaApp::default_root()
        .askama_root(TestRoot("typed-root"))
        .build()
        .unwrap();
    let html = body(response(inertia, false).await).await;
    assert!(html.contains("<meta name=\"root\" content=\"typed-root\">"));
    assert!(html.contains("<body data-root=\"askama\">"));
    assert!(html.contains(r"\u003C/script\u003E\u003Cscript\u003Ealert(1)\u003C/script\u003E"));
    assert_eq!(html.matches("data-page=\"app\"").count(), 1);
}

#[tokio::test]
async fn inertia_request_bypasses_askama_root() {
    let inertia = InertiaApp::default_root()
        .askama_root(TestRoot("typed-root"))
        .build()
        .unwrap();
    let response = response(inertia, true).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[CONTENT_TYPE], "application/json");
    let json = body(response).await;
    assert!(json.starts_with(r#"{"component":"Home""#));
    assert!(!json.contains("typed-root"));
    assert!(!json.contains("data-root"));
}

#[tokio::test]
async fn askama_and_marker_roots_use_last_call_wins() {
    let askama = InertiaApp::default_root()
        .root_template_source(MARKER_TEMPLATE)
        .askama_root(TestRoot("typed-root"))
        .build()
        .unwrap();
    assert!(
        body(response(askama, false).await)
            .await
            .contains("data-root=\"askama\"")
    );

    let marker = InertiaApp::default_root()
        .askama_root(TestRoot("typed-root"))
        .root_template_source(MARKER_TEMPLATE)
        .build()
        .unwrap();
    assert!(
        body(response(marker, false).await)
            .await
            .contains("data-root=\"marker\"")
    );
}

#[tokio::test]
async fn existing_custom_root_remains_valid() {
    let html = body(response(InertiaApp::builder(CustomRoot).build().unwrap(), false).await).await;
    assert!(html.starts_with("<custom>"));
    assert!(html.contains("data-page=\"app\""));
}

#[cfg(feature = "ssr")]
mod ssr {
    use super::*;
    use axum::routing::post;
    use inertia_axum::Ssr;

    async fn backend(status: StatusCode, response: &'static str) -> String {
        let app = Router::new()
            .route("/health", get(|| async { StatusCode::OK }))
            .route("/render", post(move || async move { (status, response) }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{address}")
    }

    #[tokio::test]
    async fn successful_ssr_uses_askama_head_and_mount_once() {
        let base = backend(
            StatusCode::OK,
            r#"{"head":["<title>SSR</title>"],"body":"<div id=\"app\" data-server-rendered=\"true\">SSR</div>"}"#,
        )
        .await;
        let inertia = InertiaApp::default_root()
            .askama_root(TestRoot("typed-root"))
            .ssr(Ssr::external(base))
            .start()
            .await
            .unwrap();
        let html = body(response(inertia, false).await).await;
        assert_eq!(html.matches("<title>SSR</title>").count(), 1);
        assert_eq!(html.matches("id=\"app\"").count(), 1);
        assert!(html.contains("data-server-rendered=\"true\""));
        assert!(!html.contains("data-page=\"app\""));
    }

    #[tokio::test]
    async fn non_strict_ssr_failure_falls_back_through_askama_root() {
        let base = backend(StatusCode::INTERNAL_SERVER_ERROR, "broken").await;
        let inertia = InertiaApp::default_root()
            .askama_root(TestRoot("typed-root"))
            .ssr(Ssr::external(base))
            .start()
            .await
            .unwrap();
        let response = response(inertia, false).await;
        assert_eq!(response.status(), StatusCode::OK);
        let html = body(response).await;
        assert!(html.contains("data-root=\"askama\""));
        assert!(html.contains("data-page=\"app\""));
        assert_eq!(html.matches("id=\"app\"").count(), 1);
    }
}
