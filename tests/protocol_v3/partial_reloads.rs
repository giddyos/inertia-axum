use super::support::*;
use axum::http::Method;

fn partial(data: &str) -> axum::http::Request<axum::body::Body> {
    let mut r = inertia_request(Method::GET, "/events");
    r.headers_mut().insert(
        "X-Inertia-Partial-Component",
        "Events/Index".parse().unwrap(),
    );
    r.headers_mut()
        .insert("X-Inertia-Partial-Data", data.parse().unwrap());
    r
}
#[tokio::test]
async fn partial_data_returns_requested_and_always_props() {
    let response = call(app(), partial("events")).await;
    let p = &response.page().unwrap()["props"];
    assert!(p.get("auth").is_some() && p.get("events").is_some() && p.get("categories").is_none());
    assert_eq!(p["errors"], serde_json::json!({}));
    insta::assert_json_snapshot!("partial_reload_only_events", response);
}
#[tokio::test]
async fn partial_except_precedes_data_and_component_mismatch_is_full() {
    let mut r = partial("events");
    r.headers_mut()
        .insert("X-Inertia-Partial-Except", "categories".parse().unwrap());
    let response = call(app(), r).await;
    assert!(response.page().unwrap()["props"].get("events").is_some());
    assert!(response.page().unwrap()["props"]
        .get("categories")
        .is_none());
    let mut mismatch = partial("events");
    mismatch.headers_mut().insert(
        "X-Inertia-Partial-Component",
        "Other/Component".parse().unwrap(),
    );
    let full = call(app(), mismatch).await;
    assert!(full.page().unwrap()["props"].get("categories").is_some());
    insta::assert_json_snapshot!("partial_except_precedence", response);
}
#[tokio::test]
async fn non_get_ignores_partial_filtering() {
    let mut r = partial("events");
    *r.method_mut() = Method::POST;
    let response = call(app(), r).await;
    assert!(response.page().unwrap()["props"]
        .get("categories")
        .is_some());
    insta::assert_json_snapshot!("write_ignores_partial_filtering", response);
}
