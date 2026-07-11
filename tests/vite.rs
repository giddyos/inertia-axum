use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    routing::get,
    Router,
};
use inertia_axum::{
    page, AssetContext, AssetError, AssetProvider, AssetTags, AssetVersion, DynamicPage,
    InertiaApp, Page, RouterInertiaExt, X_INERTIA, X_INERTIA_VERSION,
};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};
use tower::ServiceExt;

static NEXT_DIR: AtomicUsize = AtomicUsize::new(0);

fn fixture() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "inertia-axum-vite-{}-{}",
        std::process::id(),
        NEXT_DIR.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(path.join("dist/.vite")).unwrap();
    path
}

fn manifest(root: &Path, source: &str) {
    fs::write(root.join("dist/.vite/manifest.json"), source).unwrap();
}

async fn home() -> DynamicPage {
    page!("Home", { message: "hello" })
}

#[derive(Clone)]
struct NumericAssets {
    version: AssetVersion,
}

impl AssetProvider for NumericAssets {
    fn version(&self) -> &AssetVersion {
        &self.version
    }
    fn render_tags(&self, _context: AssetContext<'_>) -> Result<AssetTags, AssetError> {
        Ok(AssetTags::new(
            "<script src=\"/custom.js\"></script>".to_owned(),
        ))
    }
}

#[tokio::test]
async fn custom_provider_keeps_numeric_page_version() {
    let app = Router::new().route("/", get(home)).inertia(
        InertiaApp::default_root()
            .assets(NumericAssets {
                version: 42_u64.into(),
            })
            .build()
            .unwrap(),
    );
    let response = app
        .oneshot(
            Request::builder()
                .uri("/")
                .header(X_INERTIA, "true")
                .header(X_INERTIA_VERSION, "42")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let page: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(page["version"], 42);
}

#[tokio::test]
async fn production_manifest_resolves_imports_css_version_and_static_files() {
    let root = fixture();
    manifest(
        &root,
        r#"{
      "src/main.ts":{"file":"assets/main-123.js","css":["assets/main.css"],"imports":["_shared.js"]},
      "_shared.js":{"file":"assets/shared-456.js","css":["assets/shared.css"]}
    }"#,
    );
    fs::create_dir_all(root.join("dist/assets")).unwrap();
    fs::write(root.join("dist/assets/main-123.js"), "export default 1").unwrap();
    let inertia = InertiaApp::vite(&root).build().unwrap();
    let app = Router::new().route("/", get(home)).inertia(inertia);
    fs::remove_file(root.join("dist/.vite/manifest.json")).unwrap();
    let response = app
        .clone()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let html = String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(html.contains("/build/assets/main.css"));
    assert!(html.contains("/build/assets/shared.css"));
    assert!(html.contains("modulepreload"));
    assert!(html.contains("/build/assets/main-123.js"));
    let response = app
        .oneshot(
            Request::builder()
                .uri("/build/assets/main-123.js")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn development_mode_needs_no_manifest_and_renders_both_scripts() {
    let root = fixture();
    fs::remove_file(root.join("dist/.vite/manifest.json")).ok();
    let app = Router::new().route("/", get(home)).inertia(
        InertiaApp::vite(&root)
            .entry("src/app.ts")
            .dev_server("http://localhost:5173/")
            .build()
            .unwrap(),
    );
    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let html = String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(html.contains("http://localhost:5173/@vite/client"));
    assert!(html.contains("http://localhost:5173/src/app.ts"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn startup_errors_are_actionable() {
    let root = fixture();
    let missing = InertiaApp::vite(&root).build().err().unwrap().to_string();
    assert!(missing.contains("Could not read manifest"));
    manifest(&root, r#"{"src/app.ts":{"file":"app.js"}}"#);
    let entry = InertiaApp::vite(&root).build().err().unwrap().to_string();
    assert!(entry.contains("Entry \"src/main.ts\" was not found"));
    assert!(entry.contains("src/app.ts"));
    let malformed = InertiaApp::vite(&root)
        .dev_server("not a URL")
        .build()
        .err()
        .unwrap()
        .to_string();
    assert!(malformed.contains("VITE_DEV_SERVER_URL"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn asset_version_retains_scalar_json_and_normalizes_headers() {
    let string = AssetVersion::from("release-7");
    let number = AssetVersion::from(42_u64);
    assert_eq!(serde_json::to_value(&string).unwrap(), json!("release-7"));
    assert_eq!(serde_json::to_value(&number).unwrap(), json!(42));
    assert_eq!(number.header_value(), "42");
    let page = Page::new("Home", Value::Object(Default::default()), "/").version(number);
    assert_eq!(serde_json::to_value(page).unwrap()["version"], 42);
}

#[tokio::test]
async fn configured_overrides_change_manifest_and_public_paths() {
    let root = fixture();
    fs::create_dir_all(root.join("public/build/.vite")).unwrap();
    fs::write(
        root.join("public/build/.vite/manifest.json"),
        r#"{"src/app.ts":{"file":"app.js"}}"#,
    )
    .unwrap();
    fs::write(root.join("public/build/app.js"), "ok").unwrap();
    let app = Router::new().route("/", get(home)).inertia(
        InertiaApp::vite(&root)
            .entry("src/app.ts")
            .build_dir("public/build")
            .public_path("/assets")
            .build()
            .unwrap(),
    );
    let response = app
        .oneshot(
            Request::builder()
                .uri("/assets/app.js")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    fs::remove_dir_all(root).unwrap();
}
