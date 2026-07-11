#![allow(missing_docs)]

use axum::{
    body::{to_bytes, Body},
    http::{header::CONTENT_TYPE, Request, StatusCode},
};
use tower::ServiceExt as _;

async fn require_node_22() {
    let output = tokio::process::Command::new("node")
        .arg("--version")
        .output()
        .await
        .expect("production SSR tests require Node 22 or newer on PATH");
    assert!(output.status.success(), "`node --version` failed");
    let version = String::from_utf8_lossy(&output.stdout);
    let major = version
        .trim()
        .trim_start_matches('v')
        .split('.')
        .next()
        .and_then(|value| value.parse::<u64>().ok())
        .expect("Node returned an invalid version");
    assert!(
        major >= 22,
        "production SSR tests require Node 22 or newer; found {}",
        version.trim()
    );
}

async fn response_body(response: axum::response::Response) -> String {
    String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap()
}

fn extract_data_page(html: &str) -> serde_json::Value {
    let marker = "<script data-page=\"app\" type=\"application/json\">";
    let start = html.find(marker).expect("missing Inertia page script") + marker.len();
    let end = html[start..]
        .find("</script>")
        .map(|offset| start + offset)
        .expect("unterminated Inertia page script");
    serde_json::from_str(&html[start..end]).expect("invalid Inertia page JSON")
}

fn extract_module_script_path(html: &str) -> &str {
    let marker = "<script type=\"module\" src=\"";
    let start = html.find(marker).expect("missing client module script") + marker.len();
    let end = html[start..]
        .find('"')
        .map(|offset| start + offset)
        .expect("unterminated client module script path");
    &html[start..end]
}

#[tokio::test]
#[ignore = "requires a pnpm production build and Node 22 or newer"]
async fn production_example_uses_manifest_ssr_bundle_and_static_assets() {
    require_node_22().await;
    let frontend = axum_vue::frontend_root();
    let manifest = frontend.join("../public/build/.vite/manifest.json");
    let bundle = frontend.join("dist/ssr/app.js");
    assert!(
        manifest.is_file(),
        "missing {}; run scripts/test-live-ssr.sh",
        manifest.display()
    );
    assert!(
        bundle.is_file(),
        "missing {}; run scripts/test-live-ssr.sh",
        bundle.display()
    );

    let inertia = axum_vue::build_inertia().await.unwrap();
    let app = axum_vue::router(axum_vue::seeded_state(), inertia);
    let response = app
        .clone()
        .oneshot(Request::get("/todos").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response
        .headers()
        .get(CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("text/html"));
    let html = response_body(response).await;
    assert!(html.contains("data-server-rendered=\"true\""));
    assert!(html.contains("data-page=\"app\""));
    assert!(html.contains("<h1>Todos</h1>"));
    assert!(html.contains("Try automatic deferred props"));
    assert!(html.contains("/public/build/"));
    assert_eq!(html.matches("id=\"app\"").count(), 1);

    let page = extract_data_page(&html);
    assert_eq!(page["component"], "Todos/Index");
    assert_eq!(page["url"], "/todos");
    assert_eq!(
        page["props"]["todos"][0]["title"],
        "Try automatic deferred props"
    );
    assert!(page["version"].is_string());

    let client_asset = extract_module_script_path(&html);
    assert!(client_asset.starts_with("/public/build/"));
    let asset_response = app
        .clone()
        .oneshot(Request::get(client_asset).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(asset_response.status(), StatusCode::OK);

    let private_html = response_body(
        app.clone()
            .oneshot(Request::get("/todos/private").body(Body::empty()).unwrap())
            .await
            .unwrap(),
    )
    .await;
    assert!(!private_html.contains("data-server-rendered=\"true\""));
    assert!(!private_html.contains("<h1>Todos</h1>"));
    assert_eq!(extract_data_page(&private_html)["component"], "Todos/Index");

    let preview_html = response_body(
        app.clone()
            .oneshot(Request::get("/todos/preview").body(Body::empty()).unwrap())
            .await
            .unwrap(),
    )
    .await;
    assert!(preview_html.contains("data-server-rendered=\"true\""));
    assert!(preview_html.contains("<h1>Todos</h1>"));

    let forced_csr_html = response_body(
        app.oneshot(
            Request::get("/todos/preview")
                .header("x-force-csr", "1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap(),
    )
    .await;
    assert!(!forced_csr_html.contains("data-server-rendered=\"true\""));
    assert!(!forced_csr_html.contains("<h1>Todos</h1>"));
    assert_eq!(forced_csr_html.matches("id=\"app\"").count(), 1);
    assert_eq!(
        extract_data_page(&forced_csr_html)["component"],
        "Todos/Index"
    );
}
