use axum::{
    body::{to_bytes, Body},
    http::{Method, Request, StatusCode},
    routing::{get, post},
    Router,
};
use inertia_axum::{
    always, defer, lazy, merge, once, optional, page, scroll, ErrorHandler, InertiaApp, LoadPolicy,
    MergePolicy, PropError, RouterInertiaExt, ScrollPage, X_INERTIA, X_INERTIA_EXCEPT_ONCE_PROPS,
    X_INERTIA_INFINITE_SCROLL_MERGE_INTENT, X_INERTIA_PARTIAL_COMPONENT, X_INERTIA_PARTIAL_DATA,
    X_INERTIA_PARTIAL_EXCEPT, X_INERTIA_RESET,
};
use serde_json::{json, Value};
use std::{
    io,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, UNIX_EPOCH},
};
use tokio::sync::Barrier;
use tower::ServiceExt;

fn request(method: Method, headers: &[(&str, &str)]) -> Request<Body> {
    let mut request = Request::builder()
        .method(method)
        .uri("/")
        .header(X_INERTIA, "true");
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    request.body(Body::empty()).unwrap()
}

async fn json_page(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
}

#[test]
fn focused_helpers_share_one_composable_policy_type() {
    let prop = defer(|| async { Ok::<_, io::Error>(vec![1]) })
        .group("summary")
        .rescue()
        .once()
        .key("summary:v2")
        .append()
        .match_on("id");
    assert!(matches!(prop.options().load, LoadPolicy::Deferred { .. }));
    assert!(prop.options().once.is_some());
    assert!(matches!(
        prop.options().merge,
        Some(MergePolicy::Append { .. })
    ));
    assert!(prop.options().rescue);
}

#[tokio::test]
async fn unselected_resolvers_do_not_construct_their_futures() {
    let constructed = Arc::new(AtomicUsize::new(0));
    let counter = constructed.clone();
    let app = Router::new().route("/", get(move || {
        let counter = counter.clone();
        async move { page!("Dashboard", {
            user: "Ada",
            audit: optional(move || { counter.fetch_add(1, Ordering::SeqCst); async { Ok::<_, io::Error>(vec![1]) } }),
        }) }
    })).inertia(InertiaApp::default_root().build().unwrap());
    let page = json_page(app.oneshot(request(Method::GET, &[])).await.unwrap()).await;
    assert!(page["props"].get("audit").is_none());
    assert_eq!(constructed.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn selected_async_resolvers_run_concurrently() {
    let barrier = Arc::new(Barrier::new(2));
    let app = Router::new().route("/", get(move || {
        let one = barrier.clone();
        let two = barrier.clone();
        async move { page!("Dashboard", {
            first: lazy(move || async move { one.wait().await; Ok::<_, io::Error>(1) }),
            second: lazy(move || async move { two.wait().await; Ok::<_, io::Error>(2) }),
        }) }
    })).inertia(InertiaApp::default_root().build().unwrap());
    let response = tokio::time::timeout(
        Duration::from_secs(1),
        app.oneshot(request(Method::GET, &[])),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(json_page(response).await["props"]["second"], 2);
}

#[tokio::test]
async fn optional_deferred_always_and_partial_precedence_match_protocol() {
    async fn page_handler() -> inertia_axum::DynamicPage {
        page!("Dashboard", {
            base: 1,
            csrf: always("token"),
            audit: optional(|| async { Ok::<_, io::Error>(2) }),
            stats: defer(|| async { Ok::<_, io::Error>(3) }).group("dashboard"),
        })
    }
    let app = Router::new()
        .route("/", get(page_handler))
        .inertia(InertiaApp::default_root().build().unwrap());
    let initial = json_page(
        app.clone()
            .oneshot(request(Method::GET, &[]))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(
        initial["props"],
        json!({"base":1,"csrf":"token","errors":{}})
    );
    assert_eq!(initial["deferredProps"], json!({"dashboard":["stats"]}));
    let partial = json_page(
        app.clone()
            .oneshot(request(
                Method::GET,
                &[
                    (X_INERTIA_PARTIAL_COMPONENT, "Dashboard"),
                    (X_INERTIA_PARTIAL_DATA, "audit,stats"),
                    (X_INERTIA_PARTIAL_EXCEPT, "audit"),
                ],
            ))
            .await
            .unwrap(),
    )
    .await;
    assert!(partial["props"].get("audit").is_none());
    assert_eq!(partial["props"]["stats"], 3);
    assert_eq!(partial["props"]["csrf"], "token");
}

#[derive(Clone)]
struct Reports(Arc<Mutex<Vec<String>>>);
impl ErrorHandler for Reports {
    fn handle(&self, prop: &str, _error: &PropError) {
        self.0.lock().unwrap().push(prop.to_owned());
    }
}

#[tokio::test]
async fn rescued_failures_are_omitted_reported_and_deterministic() {
    let reports = Reports(Arc::new(Mutex::new(Vec::new())));
    let observed = reports.0.clone();
    async fn handler() -> inertia_axum::DynamicPage {
        page!("Dashboard", {
            first: defer(|| async { Err::<u32, _>(io::Error::other("first")) }).rescue(),
            second: defer(|| async { Err::<u32, _>(io::Error::other("second")) }).rescue(),
        })
    }
    let app = Router::new().route("/", get(handler)).inertia(
        InertiaApp::default_root()
            .error_handler(reports)
            .build()
            .unwrap(),
    );
    let page = json_page(
        app.clone()
            .oneshot(request(
                Method::GET,
                &[
                    (X_INERTIA_PARTIAL_COMPONENT, "Dashboard"),
                    (X_INERTIA_PARTIAL_DATA, "first,second"),
                ],
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(page["rescuedProps"], json!(["first", "second"]));
    assert!(page["props"].get("first").is_none());
    assert_eq!(*observed.lock().unwrap(), vec!["first", "second"]);
}

#[tokio::test]
async fn once_exclusions_fresh_refresh_and_expiration_compose() {
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let app = Router::new().route("/", get(move || {
        let count = count.clone();
        async move { page!("Plans", {
            plans: once(move || async move { count.fetch_add(1, Ordering::SeqCst); Ok::<_, io::Error>(vec!["pro"]) })
                .key("plans:v2").expires_at(UNIX_EPOCH + Duration::from_secs(10)),
            fresh: once(|| async { Ok::<_, io::Error>(7) }).key("fresh:v1").fresh_if(true),
        }) }
    })).inertia(InertiaApp::default_root().build().unwrap());
    let page = json_page(
        app.clone()
            .oneshot(request(
                Method::GET,
                &[(X_INERTIA_EXCEPT_ONCE_PROPS, "plans:v2,fresh:v1")],
            ))
            .await
            .unwrap(),
    )
    .await;
    assert!(page["props"].get("plans").is_none());
    assert_eq!(page["props"]["fresh"], 7);
    assert_eq!(page["onceProps"]["fresh:v1"]["prop"], "fresh");
    assert_eq!(page["onceProps"]["plans:v2"]["expiresAt"], 10_000);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let refreshed = json_page(
        app.oneshot(request(
            Method::GET,
            &[
                (X_INERTIA_PARTIAL_COMPONENT, "Plans"),
                (X_INERTIA_PARTIAL_DATA, "plans"),
                (X_INERTIA_EXCEPT_ONCE_PROPS, "plans:v2"),
            ],
        ))
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(refreshed["props"]["plans"], json!(["pro"]));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn scroll_intent_and_reset_are_applied_after_async_resolution() {
    async fn handler() -> inertia_axum::DynamicPage {
        page!("Feed", { feed: scroll(ScrollPage::new(vec![1], 2)) })
    }
    let app = Router::new()
        .route("/", get(handler))
        .inertia(InertiaApp::default_root().build().unwrap());
    let prepended = json_page(
        app.clone()
            .oneshot(request(
                Method::GET,
                &[
                    (X_INERTIA_PARTIAL_COMPONENT, "Feed"),
                    (X_INERTIA_PARTIAL_DATA, "feed"),
                    (X_INERTIA_INFINITE_SCROLL_MERGE_INTENT, "prepend"),
                ],
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(prepended["prependProps"], json!(["feed.data"]));
    assert!(prepended.get("mergeProps").is_none());
    let reset = json_page(
        app.oneshot(request(
            Method::GET,
            &[
                (X_INERTIA_PARTIAL_COMPONENT, "Feed"),
                (X_INERTIA_PARTIAL_DATA, "feed"),
                (X_INERTIA_RESET, "feed"),
            ],
        ))
        .await
        .unwrap(),
    )
    .await;
    assert!(reset.get("mergeProps").is_none());
    assert!(reset.get("scrollProps").is_none());
}

#[tokio::test]
async fn merge_and_scroll_policies_use_existing_metadata_engine() {
    async fn handler() -> inertia_axum::DynamicPage {
        page!("Feed", {
            users: merge(json!({"data":[{"id":1}]})).append_at("data").match_on_at("data", "id"),
            chat: merge(json!({"messages":[]})).deep().match_on("messages.id"),
            events: merge(json!([{"id":2}])).prepend().match_on("id"),
            feed: scroll(ScrollPage::new(vec![json!({"id":1})], 2).previous(1).next(3).page_name("feed")).match_on("id"),
        })
    }
    let app = Router::new()
        .route("/", get(handler))
        .inertia(InertiaApp::default_root().build().unwrap());
    let page = json_page(app.oneshot(request(Method::GET, &[])).await.unwrap()).await;
    assert_eq!(page["mergeProps"], json!(["users.data", "feed.data"]));
    assert_eq!(page["deepMergeProps"], json!(["chat"]));
    assert_eq!(page["prependProps"], json!(["events"]));
    assert_eq!(
        page["matchPropsOn"],
        json!(["users.data.id", "chat.messages.id", "events.id", "feed.id"])
    );
    assert_eq!(page["scrollProps"]["feed"]["pageName"], "feed");
}

#[tokio::test]
async fn writes_ignore_partial_filtering_but_keep_once_exclusions() {
    async fn handler() -> inertia_axum::DynamicPage {
        page!("Write", { ordinary: 1, cached: once(|| async { Ok::<_, io::Error>(2) }).key("cached") })
    }
    let app = Router::new()
        .route("/", post(handler))
        .inertia(InertiaApp::default_root().build().unwrap());
    let page = json_page(
        app.oneshot(request(
            Method::POST,
            &[
                (X_INERTIA_PARTIAL_COMPONENT, "Write"),
                (X_INERTIA_PARTIAL_DATA, "nothing"),
                (X_INERTIA_EXCEPT_ONCE_PROPS, "cached"),
            ],
        ))
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(page["props"]["ordinary"], 1);
    assert!(page["props"].get("cached").is_none());
}

#[tokio::test]
async fn an_unrescued_failure_becomes_a_standard_error_response() {
    async fn handler() -> inertia_axum::DynamicPage {
        page!("Failure", { value: lazy(|| async { Err::<u32, _>(io::Error::other("boom")) }) })
    }
    let app = Router::new()
        .route("/", get(handler))
        .inertia(InertiaApp::default_root().build().unwrap());
    assert_eq!(
        app.oneshot(request(Method::GET, &[]))
            .await
            .unwrap()
            .status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
}
