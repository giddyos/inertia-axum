use super::support::*;
use axum::http::Method;

#[tokio::test]
async fn page_serializes_all_supported_v3_metadata() {
    let response = call(app(), inertia_request(Method::GET, "/advanced")).await;
    let p = response.page().unwrap();
    assert_eq!(p["encryptHistory"], true);
    assert_eq!(p["mergeProps"], serde_json::json!(["posts", "posts.data"]));
    assert_eq!(p["onceProps"]["feature-catalog"]["expiresAt"], EXPIRED_AT);
    insta::assert_json_snapshot!("page_all_v3_metadata", response);
}
#[tokio::test]
async fn metadata_for_filtered_props_is_removed_and_rescued_metadata_serializes() {
    let mut r = inertia_request(Method::GET, "/advanced");
    r.headers_mut()
        .insert("X-Inertia-Partial-Component", "Feed/Index".parse().unwrap());
    r.headers_mut()
        .insert("X-Inertia-Partial-Data", "permissions".parse().unwrap());
    let response = call(app(), r).await;
    let p = response.page().unwrap();
    assert!(p.get("mergeProps").is_none());
    assert_eq!(p["rescuedProps"], serde_json::json!(["permissions"]));
    insta::assert_json_snapshot!("filtered_metadata_and_rescued_props", response);
}
