use super::support::*;
use axum::http::Method;

#[tokio::test]
async fn object_props_receive_or_preserve_errors_through_partial_reloads() {
    let basic = call(app(), inertia_request(Method::GET, "/events")).await;
    assert_eq!(
        basic.page().unwrap()["props"]["errors"],
        serde_json::json!({})
    );
    let mut r = inertia_request(Method::GET, "/errors");
    r.headers_mut()
        .insert("X-Inertia-Partial-Component", "Errors".parse().unwrap());
    r.headers_mut()
        .insert("X-Inertia-Partial-Data", "users".parse().unwrap());
    let response = call(app(), r).await;
    assert_eq!(
        response.page().unwrap()["props"]["errors"]["email"],
        "Invalid"
    );
    insta::assert_json_snapshot!("errors_always_present", response);
}
#[tokio::test]
async fn error_bag_header_is_exposed_without_automatic_shape_mutation() {
    let mut r = inertia_request(Method::GET, "/context");
    r.headers_mut()
        .insert("X-Inertia-Error-Bag", "createUser".parse().unwrap());
    let response = call(app(), r).await;
    assert_eq!(response.page().unwrap()["props"]["errorBag"], "createUser");
    insta::assert_json_snapshot!("error_bag_context", response);
}
