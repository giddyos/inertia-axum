#![allow(missing_docs)]

use http::{HeaderMap, HeaderValue, StatusCode as CoreStatus};
use inertia_embed::{EmbeddedAsset, EmbeddedFrontend, EmbeddedStorage};
use inertia_rocket::{
    AssetContext, AssetError, AssetProvider, AssetSource, AssetTags, AssetVersion, CoreBody,
    CoreResponse, DirectoryAssetSource, DynamicPage, Inertia, InertiaApp, InertiaFairing, Response,
    Result as InertiaResult,
};
use rocket::{
    Request,
    http::{Header, Status},
    local::asynchronous::Client,
    response::{Responder, Response as RocketResponse},
};
use std::{
    fs,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

static ASSETS: &[EmbeddedAsset] = &[EmbeddedAsset {
    path: "assets/app.js",
    bytes: b"console.log('rocket')",
    storage: EmbeddedStorage::Identity,
    content_type: "text/javascript; charset=utf-8",
    etag: "\"rocket-etag\"",
    immutable: false,
    encoding: None,
}];

static FRONTEND: EmbeddedFrontend = EmbeddedFrontend::new(
    "/runtime-assets",
    "src/main.ts",
    "rocket-v1",
    r#"<script type="module" src="/runtime-assets/assets/app.js"></script>"#,
    ASSETS,
);

static VERSION_HANDLER_CALLS: AtomicUsize = AtomicUsize::new(0);
static NEXT_ASSET_FIXTURE: AtomicUsize = AtomicUsize::new(0);

struct AssetFixture(PathBuf);

impl AssetFixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "inertia-rocket-assets-{}-{}",
            std::process::id(),
            NEXT_ASSET_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(path.join("assets")).unwrap();
        fs::write(
            path.join("assets/filesystem.js"),
            b"console.log('filesystem')",
        )
        .unwrap();
        Self(path)
    }
}

impl Drop for AssetFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Clone)]
struct FilesystemProvider {
    source: DirectoryAssetSource,
}

impl AssetProvider for FilesystemProvider {
    fn version(&self) -> AssetVersion {
        "filesystem-v1".into()
    }

    fn render_tags(&self, _context: AssetContext<'_>) -> Result<AssetTags, AssetError> {
        Ok(AssetTags::new(String::new()))
    }

    fn source(&self) -> Option<Arc<dyn AssetSource>> {
        Some(Arc::new(self.source.clone()))
    }

    fn public_path(&self) -> Option<&str> {
        Some("/filesystem-assets")
    }
}

fn app() -> InertiaApp {
    InertiaApp::embedded(&FRONTEND)
        .build()
        .expect("valid embedded Rocket app")
}

#[rocket::get("/render")]
async fn render(inertia: Inertia<'_>) -> InertiaResult {
    inertia
        .render(
            "Home",
            inertia_rocket::__private::Value::Object(
                [(
                    "message".to_owned(),
                    inertia_rocket::__private::Value::String("async".to_owned()),
                )]
                .into_iter()
                .collect(),
            ),
        )
        .await
}

#[rocket::get("/direct")]
fn direct() -> DynamicPage {
    DynamicPage::new("Direct").prop("answer", 42_u32)
}

#[rocket::get("/version")]
fn version(_inertia: Inertia<'_>) -> DynamicPage {
    VERSION_HANDLER_CALLS.fetch_add(1, Ordering::SeqCst);
    DynamicPage::new("Version")
}

#[rocket::get("/plain")]
fn plain() -> Plain {
    Plain
}

struct Plain;

impl<'r, 'o: 'r> Responder<'r, 'o> for Plain {
    fn respond_to(self, request: &'r Request<'_>) -> rocket::response::Result<'o> {
        RocketResponse::build_from("ordinary".respond_to(request)?)
            .status(Status::Accepted)
            .header(Header::new("X-Ordinary", "preserved"))
            .ok()
    }
}

#[rocket::get("/repeated")]
fn repeated() -> Response {
    let mut headers = HeaderMap::new();
    headers.append("set-cookie", HeaderValue::from_static("one=1"));
    headers.append("set-cookie", HeaderValue::from_static("two=2"));
    Response(CoreResponse::new(CoreStatus::OK, headers, CoreBody::Empty))
}

async fn client() -> Client {
    Client::untracked(rocket::build().attach(InertiaFairing::new(app())).mount(
        "/",
        rocket::routes![render, direct, version, plain, repeated],
    ))
    .await
    .expect("Rocket Inertia app must ignite")
}

#[rocket::async_test]
async fn awaited_guard_render_and_direct_responder_finalize_through_core() {
    let client = client().await;
    let response = client
        .get("/render")
        .header(Header::new("X-Inertia", "true"))
        .header(Header::new("X-Inertia-Version", "rocket-v1"))
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    let page: serde_json::Value =
        serde_json::from_slice(&response.into_bytes().await.unwrap()).unwrap();
    assert_eq!(page["component"], "Home");
    assert_eq!(page["props"]["message"], "async");

    let response = client
        .get("/direct")
        .header(Header::new("X-Inertia", "true"))
        .header(Header::new("X-Inertia-Version", "rocket-v1"))
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    let page: serde_json::Value =
        serde_json::from_slice(&response.into_bytes().await.unwrap()).unwrap();
    assert_eq!(page["component"], "Direct");
    assert_eq!(page["props"]["answer"], 42);
}

#[rocket::async_test]
async fn guard_version_mismatch_short_circuits_the_handler() {
    VERSION_HANDLER_CALLS.store(0, Ordering::SeqCst);
    let client = client().await;
    let response = client
        .get("/version")
        .header(Header::new("X-Inertia", "true"))
        .header(Header::new("X-Inertia-Version", "stale"))
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Conflict);
    assert_eq!(
        response.headers().get_one("X-Inertia-Location"),
        Some("/version")
    );
    assert_eq!(VERSION_HANDLER_CALLS.load(Ordering::SeqCst), 0);

    let response = client
        .get("/version")
        .header(Header::new("X-Inertia", "invalid\nvalue"))
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::BadRequest);
    assert_eq!(VERSION_HANDLER_CALLS.load(Ordering::SeqCst), 0);
}

#[rocket::async_test]
async fn fairing_mounts_assets_at_the_runtime_path_and_returns_explicit_405() {
    let client = client().await;
    let response = client.get("/runtime-assets/assets/app.js").dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    assert_eq!(
        response.into_bytes().await.unwrap(),
        b"console.log('rocket')"
    );

    let response = client
        .post("/runtime-assets/assets/app.js")
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::MethodNotAllowed);
    assert_eq!(response.headers().get_one("Allow"), Some("GET, HEAD"));
}

#[rocket::async_test]
async fn fairing_serves_filesystem_assets_through_the_same_runtime_route() {
    let fixture = AssetFixture::new();
    let provider = FilesystemProvider {
        source: DirectoryAssetSource::new(&fixture.0).unwrap(),
    };
    let inertia = InertiaApp::embedded(provider).build().unwrap();
    let client = Client::untracked(rocket::build().attach(InertiaFairing::new(inertia)))
        .await
        .expect("filesystem-backed Rocket app must ignite");

    let response = client
        .get("/filesystem-assets/assets/filesystem.js")
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    assert_eq!(
        response.into_bytes().await.unwrap(),
        b"console.log('filesystem')"
    );

    let response = client
        .head("/filesystem-assets/assets/filesystem.js")
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    assert!(response.into_bytes().await.is_none());
}

#[rocket::async_test]
async fn ordinary_and_repeated_finalized_response_headers_are_preserved() {
    let client = client().await;
    let response = client.get("/plain").dispatch().await;
    assert_eq!(response.status(), Status::Accepted);
    assert_eq!(response.headers().get_one("X-Ordinary"), Some("preserved"));
    assert_eq!(response.into_string().await.as_deref(), Some("ordinary"));

    let response = client.get("/repeated").dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    assert_eq!(
        response.headers().get("Set-Cookie").collect::<Vec<_>>(),
        ["one=1", "two=2"]
    );
}

#[rocket::async_test]
async fn missing_and_duplicate_installation_fail_deterministically() {
    let missing = Client::untracked(rocket::build().mount("/", rocket::routes![render]))
        .await
        .expect("deliberately uninstalled app must otherwise ignite");
    assert_eq!(
        missing.get("/render").dispatch().await.status(),
        Status::InternalServerError
    );

    let inertia = app();
    let duplicate = Client::untracked(
        rocket::build()
            .attach(InertiaFairing::new(inertia.clone()))
            .attach(InertiaFairing::new(inertia)),
    )
    .await;
    match duplicate {
        Ok(_) => panic!("duplicate fairing must abort ignition"),
        Err(error) => {
            let message = error.to_string();
            assert!(!message.is_empty());
        }
    }
}
