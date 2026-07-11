#![cfg(feature = "ssr")]

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Method, Request},
    response::IntoResponse,
    routing::{get, post},
};
use inertia_axum::{SsrOverride, SsrRouteExt};
use tower::ServiceExt as _;

async fn marker(router: Router, request: Request<Body>) -> Option<SsrOverride> {
    router
        .oneshot(request)
        .await
        .unwrap()
        .extensions()
        .get()
        .copied()
}

fn get_request(uri: &str) -> Request<Body> {
    Request::builder().uri(uri).body(Body::empty()).unwrap()
}

#[tokio::test]
async fn unmarked_route_inherits_global_ssr_enabled_default() {
    assert_eq!(
        marker(Router::new().route("/", get(|| async {})), get_request("/")).await,
        None
    );
}

#[tokio::test]
async fn method_route_without_ssr_is_written_to_response() {
    let app = Router::new().route("/", get(|| async {}).without_ssr());
    assert_eq!(
        marker(app, get_request("/")).await,
        Some(SsrOverride::Disabled)
    );
}

#[tokio::test]
async fn method_route_with_ssr_is_written_to_response() {
    let app = Router::new().route("/", get(|| async {}).with_ssr());
    assert_eq!(
        marker(app, get_request("/")).await,
        Some(SsrOverride::Enabled)
    );
}

#[tokio::test]
async fn router_group_policy_is_written_to_response() {
    let app = Router::new().route("/", get(|| async {})).without_ssr();
    assert_eq!(
        marker(app, get_request("/")).await,
        Some(SsrOverride::Disabled)
    );
}

#[tokio::test]
async fn method_route_override_wins_over_router_group() {
    let app = Router::new()
        .route("/", get(|| async {}).with_ssr())
        .without_ssr();
    assert_eq!(
        marker(app, get_request("/")).await,
        Some(SsrOverride::Enabled)
    );
}

#[tokio::test]
async fn nested_router_override_wins_over_outer_router() {
    let nested = Router::new().route("/page", get(|| async {})).with_ssr();
    let app = Router::new().nest("/nested", nested).without_ssr();
    assert_eq!(
        marker(app, get_request("/nested/page")).await,
        Some(SsrOverride::Enabled)
    );
}

#[tokio::test]
async fn outer_policy_does_not_replace_existing_policy() {
    let inner = Router::new().route("/", get(|| async {}).without_ssr());
    let app = Router::new().merge(inner).with_ssr();
    assert_eq!(
        marker(app, get_request("/")).await,
        Some(SsrOverride::Disabled)
    );
}

#[tokio::test]
async fn ssr_when_true_enables_ssr() {
    let app = Router::new().route("/", get(|| async {}).ssr_when(|_| true));
    assert_eq!(
        marker(app, get_request("/")).await,
        Some(SsrOverride::Enabled)
    );
}

#[tokio::test]
async fn ssr_when_false_disables_ssr() {
    let app = Router::new().route("/", get(|| async {}).ssr_when(|_| false));
    assert_eq!(
        marker(app, get_request("/")).await,
        Some(SsrOverride::Disabled)
    );
}

#[tokio::test]
async fn ssr_when_can_read_headers_uri_and_method() {
    let app = Router::new().route(
        "/inspect",
        get(|| async {}).ssr_when(|context| {
            context.method() == Method::GET
                && context.uri().query() == Some("ssr=1")
                && context.headers().contains_key("x-enable-ssr")
        }),
    );
    let request = Request::builder()
        .uri("/inspect?ssr=1")
        .header("x-enable-ssr", "yes")
        .body(Body::empty())
        .unwrap();
    assert_eq!(marker(app, request).await, Some(SsrOverride::Enabled));
}

#[tokio::test]
async fn ssr_when_can_read_request_extensions() {
    #[derive(Clone)]
    struct Feature(bool);
    let app = Router::new().route(
        "/",
        get(|| async {})
            .ssr_when(|context| context.extension::<Feature>().is_some_and(|value| value.0)),
    );
    let mut request = get_request("/");
    request.extensions_mut().insert(Feature(true));
    assert_eq!(marker(app, request).await, Some(SsrOverride::Enabled));
}

#[tokio::test]
async fn ssr_when_does_not_consume_or_modify_request_body() {
    let app = Router::new().route(
        "/",
        post(|body: String| async move { body })
            .ssr_when(|context| context.method() == Method::POST),
    );
    let response = app
        .oneshot(Request::post("/").body(Body::from("untouched")).unwrap())
        .await
        .unwrap();
    assert_eq!(response.extensions().get(), Some(&SsrOverride::Enabled));
    assert_eq!(
        to_bytes(response.into_body(), usize::MAX).await.unwrap(),
        "untouched"
    );
}

#[tokio::test]
async fn policy_does_not_change_non_inertia_response_body() {
    let app = Router::new().route("/", get(|| async { "ordinary" }).without_ssr());
    let response = app.oneshot(get_request("/")).await.unwrap();
    assert_eq!(response.status(), "ordinary".into_response().status());
    assert_eq!(
        to_bytes(response.into_body(), usize::MAX).await.unwrap(),
        "ordinary"
    );
}
