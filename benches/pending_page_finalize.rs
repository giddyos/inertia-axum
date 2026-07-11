use axum::{body::Body, http::Request, routing::get, Router};
use criterion::{criterion_group, criterion_main, Criterion};
use inertia_axum::{page, InertiaApp, RouterInertiaExt, X_INERTIA};
use tower::ServiceExt;

fn benchmark(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().expect("benchmark runtime");
    let app = Router::new()
        .route(
            "/",
            get(|| async {
                page!("Dashboard", {
                    user: serde_json::json!({"id": 1, "name": "Ada"}),
                    projects: serde_json::json!([1, 2, 3]),
                })
            }),
        )
        .inertia(InertiaApp::default_root().build().unwrap());

    c.bench_function("pending_page_finalize/inertia_json", |b| {
        b.iter(|| {
            runtime.block_on(
                app.clone().oneshot(
                    Request::builder()
                        .uri("/")
                        .header(X_INERTIA, "true")
                        .body(Body::empty())
                        .unwrap(),
                ),
            )
        });
    });
    c.bench_function("pending_page_finalize/initial_html", |b| {
        b.iter(|| {
            runtime.block_on(
                app.clone()
                    .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap()),
            )
        });
    });
}

criterion_group!(benches, benchmark);
criterion_main!(benches);
