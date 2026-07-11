use axum::{body::Body, http::Request, routing::get, Router};
use criterion::{criterion_group, criterion_main, Criterion};
use inertia_axum::{
    optional, DynamicPage, InertiaApp, RouterInertiaExt, X_INERTIA, X_INERTIA_PARTIAL_COMPONENT,
    X_INERTIA_PARTIAL_DATA,
};
use std::io;
use tower::ServiceExt;

fn benchmark(c: &mut Criterion) {
    let app = Router::new()
        .route(
            "/",
            get(|| async {
                (0..128).fold(DynamicPage::new("Bench"), |page, index| {
                    page.async_prop(
                        format!("prop{index}"),
                        optional(move || async move { Ok::<_, io::Error>(index) }),
                    )
                })
            }),
        )
        .inertia(InertiaApp::default_root().build().unwrap());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    c.bench_function("partial_async_prop_selection", |b| {
        b.iter(|| {
            runtime.block_on(
                app.clone().oneshot(
                    Request::builder()
                        .uri("/")
                        .header(X_INERTIA, "true")
                        .header(X_INERTIA_PARTIAL_COMPONENT, "Bench")
                        .header(X_INERTIA_PARTIAL_DATA, "prop64")
                        .body(Body::empty())
                        .unwrap(),
                ),
            )
        })
    });
}
criterion_group!(benches, benchmark);
criterion_main!(benches);
