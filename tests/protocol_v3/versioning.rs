use super::support::*;
use axum::{
    body::Body,
    http::{Method, Request},
};
use inertia_axum::{X_INERTIA, X_INERTIA_VERSION};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[tokio::test]
async fn matching_version_reaches_handler() {
    let counter = Arc::new(AtomicUsize::new(0));
    let response = call(
        handler_counter_app(counter.clone()),
        inertia_request(Method::GET, "/handler-counter"),
    )
    .await;
    assert_eq!(response.status, 200);
    assert_eq!(counter.load(Ordering::SeqCst), 1);
    assert_eq!(response.page().unwrap()["version"], VERSION);
    insta::assert_json_snapshot!("matching_version_reaches_handler", response);
}
#[tokio::test]
async fn stale_get_returns_conflict_before_handler() {
    let counter = Arc::new(AtomicUsize::new(0));
    let mut r = inertia_request(Method::GET, "/handler-counter?tab=active");
    r.headers_mut()
        .insert(X_INERTIA_VERSION, "stale-version".parse().unwrap());
    let response = call(handler_counter_app(counter.clone()), r).await;
    assert_eq!(response.status, 409);
    assert_eq!(
        response.header("x-inertia-location"),
        Some("/handler-counter?tab=active")
    );
    assert_eq!(counter.load(Ordering::SeqCst), 0);
    insta::assert_json_snapshot!("stale_asset_version", response);
}
#[tokio::test]
async fn missing_version_conflicts_but_browser_and_writes_do_not() {
    let counter = Arc::new(AtomicUsize::new(0));
    let r = Request::builder()
        .uri("/handler-counter")
        .header(X_INERTIA, "true")
        .body(Body::empty())
        .unwrap();
    let response = call(handler_counter_app(counter.clone()), r).await;
    assert_eq!(response.status, 409);
    for method in [Method::POST, Method::PUT, Method::PATCH, Method::DELETE] {
        let mut r = inertia_request(method, "/handler-counter");
        r.headers_mut()
            .insert(X_INERTIA_VERSION, "stale".parse().unwrap());
        assert_eq!(
            call(handler_counter_app(counter.clone()), r).await.status,
            200
        );
    }
    let browser = call(
        handler_counter_app(counter),
        Request::builder()
            .uri("/handler-counter")
            .header(X_INERTIA_VERSION, "stale")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(browser.status, 200);
    insta::assert_json_snapshot!("missing_version_conflict", response);
}
#[tokio::test]
async fn dynamic_version_is_resolved_per_request_and_not_found_passthrough() {
    let n = Arc::new(AtomicUsize::new(1));
    let mut first = inertia_request(Method::GET, "/users/123");
    first
        .headers_mut()
        .insert(X_INERTIA_VERSION, "asset-1".parse().unwrap());
    let one = call(app_with_dynamic_version(n.clone()), first).await;
    assert_eq!(one.page().unwrap()["version"], "asset-1");
    let mut second = inertia_request(Method::GET, "/users/123");
    second
        .headers_mut()
        .insert(X_INERTIA_VERSION, "asset-2".parse().unwrap());
    let two = call(app_with_dynamic_version(n), second).await;
    assert_eq!(two.page().unwrap()["version"], "asset-2");
    let not_found = call(app(), inertia_request(Method::GET, "/missing")).await;
    assert_eq!(not_found.status, 404);
    insta::assert_json_snapshot!("dynamic_version", one);
}
