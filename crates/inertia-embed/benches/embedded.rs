#![allow(missing_docs)]

use axum::{Router, body::Body, http::Request, routing::get};
use criterion::{Criterion, criterion_group, criterion_main};
use http::{HeaderMap, Method, header::IF_NONE_MATCH};
use inertia_axum::{InertiaApp, RouterInertiaExt, X_INERTIA, X_INERTIA_VERSION, page};
use inertia_core::{AssetRequest, AssetSource};
use inertia_embed::{EmbeddedAsset, EmbeddedFrontend, EmbeddedStorage, embed_frontend};
use tower::ServiceExt as _;

static BYTES: &[u8] = b"export default 1";
static ASSETS: &[EmbeddedAsset] = &[EmbeddedAsset {
    path: "assets/app-12345678.js",
    bytes: BYTES,
    storage: EmbeddedStorage::Identity,
    content_type: "text/javascript; charset=utf-8",
    etag: "\"sha256-benchmark\"",
    immutable: true,
    encoding: None,
}];
static FRONTEND: EmbeddedFrontend = EmbeddedFrontend::new(
    "/build",
    "src/main.ts",
    "frontend-sha256-benchmark",
    "<script type=\"module\" src=\"/build/assets/app-12345678.js\"></script>",
    ASSETS,
);
static COMPRESSED_FRONTEND: EmbeddedFrontend = embed_frontend! {
    root: "tests/fixtures/valid/dist",
    entry: "src/main.ts",
};

fn asset_request<'a>(method: &'a Method, headers: &'a HeaderMap) -> AssetRequest<'a> {
    AssetRequest {
        method,
        path: "assets/app-12345678.js",
        headers,
    }
}

fn benchmark(criterion: &mut Criterion) {
    criterion.bench_function("embedded_asset_binary_search", |bencher| {
        bencher.iter(|| FRONTEND.find("assets/app-12345678.js"));
    });
    criterion.bench_function("embedded_asset_get", |bencher| {
        bencher.iter(|| (&FRONTEND).get(asset_request(&Method::GET, &HeaderMap::new())));
    });
    criterion.bench_function("embedded_asset_head", |bencher| {
        bencher.iter(|| (&FRONTEND).get(asset_request(&Method::HEAD, &HeaderMap::new())));
    });
    criterion.bench_function("embedded_asset_304", |bencher| {
        let mut headers = HeaderMap::new();
        headers.insert(IF_NONE_MATCH, ASSETS[0].etag.parse().unwrap());
        bencher.iter(|| (&FRONTEND).get(asset_request(&Method::GET, &headers)));
    });
    let compressed_headers = HeaderMap::new();
    criterion.bench_function("embedded_asset_cached_decompression", |bencher| {
        bencher.iter(|| {
            (&COMPRESSED_FRONTEND).get(AssetRequest {
                method: &Method::GET,
                path: "assets/repetitive-data.txt",
                headers: &compressed_headers,
            })
        });
    });

    let app = Router::new()
        .route(
            "/",
            get(|| async { page!("Home", { message: "embedded" }) }),
        )
        .with_inertia(InertiaApp::embedded(&FRONTEND).build().unwrap());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    criterion.bench_function("initial_page_with_embedded_tags", |bencher| {
        bencher.iter(|| {
            runtime.block_on(
                app.clone()
                    .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap()),
            )
        });
    });
    criterion.bench_function("embedded_inertia_json_page", |bencher| {
        bencher.iter(|| {
            runtime.block_on(
                app.clone().oneshot(
                    Request::builder()
                        .uri("/")
                        .header(X_INERTIA, "true")
                        .header(X_INERTIA_VERSION, FRONTEND.version)
                        .body(Body::empty())
                        .unwrap(),
                ),
            )
        });
    });
}

criterion_group!(benches, benchmark);
criterion_main!(benches);
