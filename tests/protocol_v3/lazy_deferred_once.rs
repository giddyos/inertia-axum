use super::support::*;
use axum::http::Method;
use axum_inertia::{
    X_INERTIA_EXCEPT_ONCE_PROPS, X_INERTIA_PARTIAL_COMPONENT, X_INERTIA_PARTIAL_DATA,
};

fn lazy_request(data: Option<&str>) -> axum::http::Request<axum::body::Body> {
    let mut r = inertia_request(Method::GET, "/lazy");
    if let Some(data) = data {
        r.headers_mut()
            .insert(X_INERTIA_PARTIAL_COMPONENT, "Dashboard".parse().unwrap());
        r.headers_mut()
            .insert(X_INERTIA_PARTIAL_DATA, data.parse().unwrap());
    }
    r
}
#[tokio::test]
async fn lazy_optional_and_deferred_props_follow_selection_rules() {
    let initial = call(app(), lazy_request(None)).await;
    let p = &initial.page().unwrap()["props"];
    assert!(
        p.get("standard").is_some() && p.get("audit").is_none() && p.get("analytics").is_none()
    );
    assert_eq!(
        initial.page().unwrap()["deferredProps"]["default"],
        serde_json::json!(["analytics", "metrics", "deferredOnce"])
    );
    let requested = call(app(), lazy_request(Some("audit,analytics"))).await;
    let p = &requested.page().unwrap()["props"];
    assert!(
        p.get("audit").is_some() && p.get("analytics").is_some() && p.get("standard").is_none()
    );
    assert_eq!(
        requested.page().unwrap()["deferredProps"]["default"],
        serde_json::json!(["metrics", "deferredOnce"])
    );
    insta::assert_json_snapshot!("deferred_initial_response", initial);
    insta::assert_json_snapshot!("requested_optional_and_deferred", requested);
}
#[tokio::test]
async fn once_props_support_exclusion_explicit_reload_and_expiration() {
    let initial = call(app(), lazy_request(None)).await;
    assert_eq!(
        initial.page().unwrap()["onceProps"]["feature-catalog"]["expiresAt"],
        EXPIRED_AT
    );
    let mut excluded = lazy_request(None);
    excluded.headers_mut().insert(
        X_INERTIA_EXCEPT_ONCE_PROPS,
        "plans,feature-catalog,deferredOnce".parse().unwrap(),
    );
    let excluded = call(app(), excluded).await;
    assert!(excluded.page().unwrap()["props"].get("plans").is_none());
    assert!(excluded.page().unwrap()["deferredProps"]
        .get("default")
        .is_none_or(|v| !v
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("deferredOnce"))));
    let mut explicit = lazy_request(Some("plans"));
    explicit
        .headers_mut()
        .insert(X_INERTIA_EXCEPT_ONCE_PROPS, "plans".parse().unwrap());
    let explicit = call(app(), explicit).await;
    assert!(explicit.page().unwrap()["props"].get("plans").is_some());
    insta::assert_json_snapshot!("once_props_excluded", excluded);
}
#[test]
fn scoped_props_can_borrow_route_data() {
    let route_data = String::from("Ada");
    let _props = axum_inertia::ScopedInertiaProps::new().lazy("user", || route_data.as_str());
}
