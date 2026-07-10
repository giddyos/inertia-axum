use super::support::*;
use axum::http::Method;

#[tokio::test]
async fn page_url_uses_path_and_query_only_and_relative_uri_references_are_accepted() {
    let absolute = call(
        app(),
        inertia_request(Method::GET, "https://example.test/users/123?tab=profile"),
    )
    .await;
    assert_eq!(absolute.page().unwrap()["url"], "/users/123?tab=profile");
    let relative = call(app(), inertia_request(Method::GET, "/relative-external")).await;
    assert_eq!(relative.status, 409);
    assert_eq!(
        relative.header("x-inertia-redirect"),
        Some("../outside?from=axum#fragment")
    );
    insta::assert_json_snapshot!("relative_location_reference", relative);
}
