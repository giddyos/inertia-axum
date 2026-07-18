//! Stateful client integration coverage.
#![allow(dead_code)]

use axum::{
    Router,
    http::{HeaderMap, header::SET_COOKIE},
    routing::get,
};
use inertia_axum::prelude::*;
use inertia_test::TestApp;

#[tokio::test]
async fn cookies_are_persisted_between_in_process_requests() {
    let router = Router::new()
        .route(
            "/set",
            get(|| async { ([(SET_COOKIE, "session=abc; Path=/")], "set") }),
        )
        .route(
            "/clear",
            get(|| async { ([(SET_COOKIE, "session=; Path=/; Max-Age=0")], "clear") }),
        )
        .route(
            "/cookie",
            get(|headers: HeaderMap| async move {
                headers
                    .get("cookie")
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or_default()
                    .to_owned()
            }),
        );
    let app = TestApp::new(router);
    app.get("/set").send().await.assert_ok();
    let response = app.get("/cookie").send().await.assert_ok();
    assert_eq!(response.body(), b"session=abc");
    app.get("/clear").send().await.assert_ok();
    assert!(
        app.get("/cookie")
            .send()
            .await
            .assert_ok()
            .body()
            .is_empty()
    );
}

#[derive(InertiaPage)]
#[inertia(component = "Versioned")]
struct VersionedPage {
    ready: bool,
}

#[tokio::test]
async fn configured_string_versions_are_sent_and_asserted() {
    let router = Router::new()
        .route(
            "/",
            get(|| async { PendingPage::typed(VersionedPage { ready: true }) }),
        )
        .inertia(
            InertiaApp::default_root()
                .version("release-a")
                .build()
                .unwrap(),
        );
    TestApp::new(router)
        .with_version("release-a")
        .inertia_get("/")
        .send()
        .await
        .assert_page::<VersionedPage>()
        .assert_version("release-a");
}
