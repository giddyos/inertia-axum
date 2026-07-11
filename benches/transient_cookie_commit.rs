use axum::{
    body::Body,
    http::{Method, Request},
    routing::post,
    Router,
};
use criterion::{criterion_group, criterion_main, Criterion};
use inertia_axum::{CookieTransient, InertiaApp, Redirect, RouterInertiaExt, X_INERTIA};
use tower::ServiceExt;

fn benchmark(c: &mut Criterion) {
    let app = Router::new()
        .route(
            "/",
            post(|| async { Redirect::to("/").flash("toast", "saved").flash("id", 42) }),
        )
        .inertia(
            InertiaApp::default_root()
                .transient(CookieTransient::encrypted([9_u8; 32]))
                .build()
                .unwrap(),
        );
    let runtime = tokio::runtime::Runtime::new().unwrap();
    c.bench_function("transient_cookie_commit", |b| {
        b.iter(|| {
            runtime.block_on(
                app.clone().oneshot(
                    Request::builder()
                        .method(Method::POST)
                        .uri("/")
                        .header(X_INERTIA, "true")
                        .body(Body::empty())
                        .unwrap(),
                ),
            )
        })
    });
}

criterion_group!(benches, benchmark);
criterion_main!(benches);
