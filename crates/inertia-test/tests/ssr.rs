#![allow(missing_docs)]

use axum::{Router, routing::get};
use inertia_axum::{DynamicPage, InertiaApp, RouterInertiaExt as _, SsrRouteExt as _};
use inertia_test::{TestApp, TestSsr, TestSsrDocument};

#[tokio::test]
async fn test_ssr_records_calls_and_supports_response_assertions() {
    let ssr = TestSsr::builder().render(
        "Home",
        TestSsrDocument::new(
            ["<title>Home</title>".to_owned()],
            r#"<script data-page="app" type="application/json">{"component":"Home"}</script><div data-server-rendered="true" id="app">Home</div>"#,
        ),
    ).start().await;
    let inertia = InertiaApp::default_root()
        .ssr(ssr.config())
        .start()
        .await
        .unwrap();
    let router = Router::new()
        .route("/", get(|| async { DynamicPage::new("Home") }))
        .route(
            "/dashboard",
            get(|| async { DynamicPage::new("Dashboard") }).without_ssr(),
        )
        .inertia(inertia);
    let app = TestApp::new(router);
    app.get("/")
        .send()
        .await
        .assert_ssr()
        .assert_ssr_head_contains("<title>Home</title>");
    app.get("/dashboard").send().await.assert_csr();
    ssr.assert_render_count(1);
    ssr.assert_rendered_component("Home");
    ssr.assert_not_rendered_component("Dashboard");
    assert_eq!(ssr.calls()[0].url(), "/");
}
