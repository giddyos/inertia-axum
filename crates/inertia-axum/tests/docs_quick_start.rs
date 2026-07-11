//! Compile- and request-tested server half of the documentation quick start.

use axum::{
    Router,
    body::{Body, to_bytes},
    http::Request,
    routing::get,
};
use inertia_axum::prelude::*;
use tower::ServiceExt;

async fn index() -> DynamicPage {
    page!("Home", { greeting: "Hello" })
}

fn documented_app(inertia: InertiaApp) -> Router {
    Router::new().route("/", get(index)).inertia(inertia)
}

#[tokio::test]
async fn quick_start_page_uses_the_documented_api() {
    let inertia = InertiaApp::default_root().build().unwrap();
    let response = documented_app(inertia)
        .oneshot(
            Request::builder()
                .uri("/")
                .header("x-inertia", "true")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let page: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(page["component"], "Home");
    assert_eq!(page["props"]["greeting"], "Hello");
}
