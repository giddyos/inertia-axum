use super::support::*;
use axum::http::Method;
use inertia_axum::axum::SharedProps;
use serde_json::json;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[tokio::test]
async fn fixed_and_request_aware_shared_props_merge_and_dedupe_roots() {
    let shared = SharedProps::new()
        .value("appName", "Demo")
        .value("auth.user", json!({"name":"Ada"}))
        .value("auth.csrf", "token");
    let app = app_with_shared_props(shared);
    let response = call(app, inertia_request(Method::GET, "/users/123")).await;
    let p = response.page().unwrap();
    assert_eq!(p["props"]["auth"]["user"]["name"], "Ada");
    assert_eq!(p["sharedProps"], json!(["appName", "auth"]));
    insta::assert_json_snapshot!("shared_props_json", response);
}
#[tokio::test]
async fn optional_collision_and_non_object_compatibility() {
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = calls.clone();
    let shared = SharedProps::new()
        .prop_optional("missing", |_| Option::<String>::None)
        .prop("user.name", move |_| {
            seen.fetch_add(1, Ordering::SeqCst);
            "Shared"
        })
        .value("appName", "Demo");
    let response = call(
        app_with_shared_props(shared),
        inertia_request(Method::GET, "/empty"),
    )
    .await;
    assert_eq!(response.page().unwrap()["props"]["appName"], "Demo");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    insta::assert_json_snapshot!("shared_props_promote_non_object", response);
    let empty = call(app(), inertia_request(Method::GET, "/empty")).await;
    assert!(empty.page().unwrap()["props"].is_null());
}
