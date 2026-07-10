use super::support::*;
use axum::http::Method;

#[tokio::test]
async fn external_locations_use_protocol_conflicts_and_fragments_redirect_header() {
    let location = call(app(), inertia_request(Method::GET, "/external")).await;
    assert_eq!(location.status, 409);
    assert_eq!(
        location.header("x-inertia-location"),
        Some("https://example.com/outside")
    );
    assert_eq!(location.header("vary"), Some("X-Inertia"));
    let fragment = call(app(), inertia_request(Method::GET, "/external-fragment")).await;
    assert_eq!(
        fragment.header("x-inertia-redirect"),
        Some("https://example.com/outside#details")
    );
    assert!(fragment.header("x-inertia-location").is_none());
    insta::assert_json_snapshot!("external_location_conflict", location);
    insta::assert_json_snapshot!("fragment_location_conflict", fragment);
}
#[tokio::test]
async fn direct_location_and_application_redirects_are_method_aware() {
    let browser = axum::http::Request::builder()
        .uri("/external")
        .body(axum::body::Body::empty())
        .unwrap();
    assert_eq!(call(app(), browser).await.status, 302);
    for method in [Method::POST, Method::PUT, Method::PATCH, Method::DELETE] {
        let r = axum::http::Request::builder()
            .method(method)
            .uri("/redirect")
            .body(axum::body::Body::empty())
            .unwrap();
        let response = call(app(), r).await;
        assert_eq!(response.status, 303);
        assert_eq!(response.header("location"), Some("/target"));
    }
    let relative = call(app(), inertia_request(Method::GET, "/relative-redirect")).await;
    assert_eq!(relative.header("location"), Some("?next=target#fragment"));
    insta::assert_json_snapshot!("relative_redirect", relative);
}
#[tokio::test]
async fn invalid_locations_and_redirects_return_generic_500() {
    for path in ["/invalid-location", "/invalid-redirect"] {
        let response = call(app(), inertia_request(Method::GET, path)).await;
        assert_eq!(response.status, 500);
        assert!(
            response.header("location").is_none()
                && response.header("x-inertia-location").is_none()
        );
        insta::assert_json_snapshot!(format!("invalid_uri_{}", &path[1..]), response);
    }
}
