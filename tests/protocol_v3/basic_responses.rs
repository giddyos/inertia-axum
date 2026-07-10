use super::support::*;
use axum::{
    body::Body,
    http::{Method, Request},
};
use inertia_axum::X_INERTIA;

#[tokio::test]
async fn initial_visit_returns_html_page() {
    let response = call(
        app(),
        Request::builder()
            .uri("/users/123?tab=profile")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(response.status, 200);
    assert_eq!(response.header("vary"), Some("X-Inertia"));
    assert!(response.header(X_INERTIA).is_none());
    let page = response.page().unwrap();
    assert_eq!(page["component"], "Users/Show");
    assert_eq!(page["url"], "/users/123?tab=profile");
    assert_eq!(page["props"]["user"]["id"], 123);
    insta::assert_json_snapshot!("initial_visit_html_page", response);
}

#[tokio::test]
async fn inertia_visit_returns_json_page() {
    let mut request = inertia_request(Method::GET, "/users/123?tab=profile");
    let headers = request.headers_mut();
    headers.insert("x-requested-with", "XMLHttpRequest".parse().unwrap());
    headers.insert(
        "accept",
        "text/html, application/xhtml+xml".parse().unwrap(),
    );
    let response = call(app(), request).await;
    assert_eq!(response.status, 200);
    assert_eq!(response.header("content-type"), Some("application/json"));
    assert_eq!(response.header(X_INERTIA), Some("true"));
    assert_eq!(response.page().unwrap()["version"], VERSION);
    insta::assert_json_snapshot!("inertia_json_page", response);
}

#[tokio::test]
async fn minimal_page_omits_empty_metadata() {
    let response = call(app(), inertia_request(Method::GET, "/users/123")).await;
    let page = response.page().unwrap();
    for key in [
        "encryptHistory",
        "clearHistory",
        "preserveFragment",
        "mergeProps",
        "prependProps",
        "deepMergeProps",
        "matchPropsOn",
        "scrollProps",
        "deferredProps",
        "rescuedProps",
        "sharedProps",
        "onceProps",
    ] {
        assert!(page.get(key).is_none(), "{key}");
    }
    insta::assert_json_snapshot!("minimal_page_omits_empty_metadata", response);
}

#[tokio::test]
async fn response_without_version_layer_omits_version() {
    let response = call(
        app_without_version_layer(),
        inertia_request(Method::GET, "/users/123"),
    )
    .await;
    assert_eq!(response.status, 200);
    assert!(response.page().unwrap().get("version").is_none());
    insta::assert_json_snapshot!("page_without_version_layer", response);
}
#[tokio::test]
async fn explicit_url_override_wins() {
    let response = call(
        app(),
        inertia_request(Method::GET, "/custom-url?ignored=true"),
    )
    .await;
    assert_eq!(response.page().unwrap()["url"], "/canonical/users/123");
    insta::assert_json_snapshot!("explicit_url_override", response);
}
#[tokio::test]
async fn absolute_form_request_uses_local_page_url() {
    let response = call(
        app(),
        inertia_request(Method::GET, "https://example.test/users/123?tab=profile"),
    )
    .await;
    assert_eq!(response.page().unwrap()["url"], "/users/123?tab=profile");
    insta::assert_json_snapshot!("absolute_form_local_page_url", response);
}
#[tokio::test]
async fn nested_router_preserves_original_uri() {
    let response = call(
        app(),
        inertia_request(Method::GET, "/nested/page?filter=active"),
    )
    .await;
    assert_eq!(
        response.page().unwrap()["url"],
        "/nested/page?filter=active"
    );
    insta::assert_json_snapshot!("nested_original_uri", response);
}
