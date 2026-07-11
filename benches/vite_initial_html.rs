use axum::{body::Body, http::Request, routing::get, Router};
use criterion::{criterion_group, criterion_main, Criterion};
use inertia_axum::{page, InertiaApp, RouterInertiaExt};
use std::fs;
use tower::ServiceExt;

fn benchmark(c: &mut Criterion) {
    let root = std::env::temp_dir().join(format!("inertia-axum-vite-bench-{}", std::process::id()));
    fs::create_dir_all(root.join("dist/.vite")).unwrap();
    fs::write(
        root.join("dist/.vite/manifest.json"),
        r#"{"src/main.ts":{"file":"assets/main.js","css":["assets/main.css"]}}"#,
    )
    .unwrap();
    let app = Router::new()
        .route("/", get(|| async { page!("Home", { message: "hello" }) }))
        .inertia(InertiaApp::vite(&root).build().unwrap());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    c.bench_function("vite_initial_html", |b| {
        b.iter(|| {
            runtime.block_on(
                app.clone()
                    .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap()),
            )
        });
    });
    fs::remove_dir_all(root).unwrap();
}

criterion_group!(benches, benchmark);
criterion_main!(benches);
