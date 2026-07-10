use super::support::*;
use axum::{body::Body, http::Request};

#[tokio::test]
async fn html_page_json_is_safe_for_script_context() {
    let response = call(
        app(),
        Request::builder()
            .uri("/unsafe")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let super::support::CapturedBody::Html {
        page,
        raw_data_page,
    } = &response.body
    else {
        panic!("expected html")
    };
    assert!(!raw_data_page.contains("</script>"));
    for escaped in ["\\u003C", "\\u003E", "\\u0026", "\\u2028", "\\u2029"] {
        assert!(raw_data_page.contains(escaped), "{escaped}");
    }
    assert_eq!(
        page["props"]["text"],
        "</script><script>alert(1)</script>&\u{2028}\u{2029}"
    );
    insta::assert_json_snapshot!("html_script_escaping", response);
}
