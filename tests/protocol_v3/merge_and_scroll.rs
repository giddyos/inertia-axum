use super::support::*;
use axum::http::Method;

fn scroll_request(
    intent: Option<&str>,
    reset: Option<&str>,
) -> axum::http::Request<axum::body::Body> {
    let mut r = inertia_request(Method::GET, "/scroll");
    r.headers_mut().insert(
        "X-Inertia-Partial-Component",
        "Scroll/Index".parse().unwrap(),
    );
    if let Some(v) = intent {
        r.headers_mut()
            .insert("X-Inertia-Infinite-Scroll-Merge-Intent", v.parse().unwrap());
    }
    if let Some(v) = reset {
        r.headers_mut()
            .insert("X-Inertia-Reset", v.parse().unwrap());
    }
    r
}
#[tokio::test]
async fn scroll_metadata_and_merge_intents_serialize() {
    let normal = call(app(), scroll_request(None, None)).await;
    assert_eq!(
        normal.page().unwrap()["mergeProps"],
        serde_json::json!(["posts.data", "other"])
    );
    let prepend = call(app(), scroll_request(Some("prepend"), None)).await;
    assert!(prepend.page().unwrap()["prependProps"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("posts.data")));
    assert!(!prepend.page().unwrap()["mergeProps"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("posts.data")));
    insta::assert_json_snapshot!("scroll_prepend_intent", prepend);
}
#[tokio::test]
async fn reset_removes_only_matching_metadata() {
    let response = call(app(), scroll_request(None, Some("posts"))).await;
    let p = response.page().unwrap();
    assert!(p["props"].get("posts").is_some());
    assert!(p.get("scrollProps").is_none());
    assert!(!p["mergeProps"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("posts.data")));
    assert!(p["mergeProps"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("other")));
    insta::assert_json_snapshot!("reset_prunes_scroll_metadata", response);
}
