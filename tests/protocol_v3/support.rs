use axum::{
    body::{to_bytes, Body},
    extract::Path,
    http::{header::CONTENT_TYPE, Method, Request},
    response::{Html, Response},
    routing::get,
    Extension, Router,
};
use inertia_axum::{
    axum::{InertiaError, InertiaRequest, SharedProps, VersionLayer},
    Inertia, InertiaProps, OnceProp, ScrollProps, X_INERTIA, X_INERTIA_VERSION,
};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use tower::ServiceExt;

pub const VERSION: &str = "asset-version-v3";
pub const EXPIRED_AT: u64 = 1_735_689_600_000;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapturedResponse {
    pub status: u16,
    pub headers: BTreeMap<String, Vec<String>>,
    pub body: CapturedBody,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub enum CapturedBody {
    Json(Value),
    Html { page: Value, raw_data_page: String },
    Text(String),
    Empty,
}

impl CapturedResponse {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .get(&name.to_ascii_lowercase())
            .and_then(|values| values.first())
            .map(String::as_str)
    }
    pub fn page(&self) -> Option<&Value> {
        match &self.body {
            CapturedBody::Json(page) => Some(page),
            CapturedBody::Html { page, .. } => Some(page),
            _ => None,
        }
    }
}

pub async fn capture(response: Response) -> CapturedResponse {
    let status = response.status().as_u16();
    let is_html = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("text/html"));
    let mut headers = BTreeMap::new();
    for name in [
        "content-type",
        "location",
        "vary",
        "x-inertia",
        "x-inertia-location",
        "x-inertia-redirect",
    ] {
        let values: Vec<_> = response
            .headers()
            .get_all(name)
            .iter()
            .filter_map(|v| v.to_str().ok().map(str::to_owned))
            .collect();
        if !values.is_empty() {
            headers.insert(name.to_owned(), values);
        }
    }
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    let body = if text.is_empty() {
        CapturedBody::Empty
    } else if is_html {
        let raw_data_page = text
            .split("<script data-page=\"app\" type=\"application/json\">")
            .nth(1)
            .and_then(|tail| tail.split("</script>").next())
            .expect("fixture must render data-page")
            .to_owned();
        let page = serde_json::from_str(&raw_data_page).expect("data-page must be JSON");
        CapturedBody::Html {
            page,
            raw_data_page,
        }
    } else if let Ok(page) = serde_json::from_str(&text) {
        CapturedBody::Json(page)
    } else {
        CapturedBody::Text(text)
    };
    CapturedResponse {
        status,
        headers,
        body,
    }
}

pub fn inertia_request(method: Method, uri: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(X_INERTIA, "true")
        .header(X_INERTIA_VERSION, VERSION)
        .body(Body::empty())
        .unwrap()
}
pub async fn call(app: Router, request: Request<Body>) -> CapturedResponse {
    capture(app.oneshot(request).await.unwrap()).await
}

fn shell(context: inertia_axum::HtmlResponseContext) -> Html<String> {
    Html(format!("<!doctype html><html><body><script data-page=\"app\" type=\"application/json\">{}</script><div id=\"app\"></div></body></html>", context.data_page()))
}

async fn users(Path(id): Path<u64>, request: InertiaRequest) -> Result<Response, InertiaError> {
    request.render(
        Inertia::response("Users/Show", json!({"user": {"id": id, "name": "Ada"}})),
        shell,
    )
}
async fn events(request: InertiaRequest) -> Result<Response, InertiaError> {
    request.render(
        Inertia::response(
            "Events/Index",
            InertiaProps::new()
                .always("auth", || json!({"user":{"name":"Ada"}}))
                .value("events", json!([1, 2]))
                .value("categories", json!(["meetups"])),
        ),
        shell,
    )
}
async fn advanced(request: InertiaRequest) -> Result<Response, InertiaError> {
    request.render(Inertia::page("Feed/Index").encrypt_history().clear_history().preserve_fragment().merge("posts").merge("posts").prepend("notifications").deep_merge("conversations").match_on("posts.id").match_on("notifications.id").match_on("conversations.data.id").scroll("posts", ScrollProps::new("page", 2).previous_page(1).next_page(3)).defer_group("default", "analytics").defer_group("sidebar", "relatedPosts").rescue("permissions").share("auth").once("plans").once_with_key("feature-catalog", OnceProp::new("features").expires_at(EXPIRED_AT)).props(json!({"posts":{"data":[1]},"notifications":[],"conversations":[],"analytics":1,"relatedPosts":[],"permissions":true,"auth":{"user":"Ada"},"plans":[],"features":[]})), shell)
}
async fn lazy(request: InertiaRequest) -> Result<Response, InertiaError> {
    request.render(
        Inertia::response(
            "Dashboard",
            InertiaProps::new()
                .value("user", json!({"name":"Ada"}))
                .lazy("standard", || "yes")
                .optional("audit", || json!(["created"]))
                .always("always", || 1)
                .defer_group("default", "analytics", || json!({"visits": 3}))
                .defer_group("default", "metrics", || 2)
                .defer_group("sidebar", "relatedPosts", || json!([]))
                .once("plans", || json!(["basic", "pro"]))
                .once_with_key(
                    "feature-catalog",
                    OnceProp::new("features").expires_at(EXPIRED_AT),
                    || json!(["search"]),
                )
                .defer_once("deferredOnce", || 7)
                .optional_once("optionalOnce", || 8),
        ),
        shell,
    )
}
async fn scroll(request: InertiaRequest) -> Result<Response, InertiaError> {
    request.render(
        Inertia::page("Scroll/Index")
            .scroll(
                "posts",
                ScrollProps::new("page", 2).previous_page(1).next_page(3),
            )
            .merge("other")
            .deep_merge("otherDeep")
            .match_on("posts.id")
            .match_on("other.id")
            .props(json!({"posts":{"data":[1,2]},"other":[],"otherDeep":[]})),
        shell,
    )
}
async fn history(request: InertiaRequest) -> Result<Response, InertiaError> {
    request.render(
        Inertia::response("History", ())
            .encrypt_history()
            .clear_history()
            .preserve_fragment(),
        shell,
    )
}
async fn errors(request: InertiaRequest) -> Result<Response, InertiaError> {
    request.render(
        Inertia::response(
            "Errors",
            InertiaProps::new()
                .lazy("errors", || json!({"email":"Invalid"}))
                .value("users", json!(["Ada"])),
        ),
        shell,
    )
}
async fn empty(request: InertiaRequest) -> Result<Response, InertiaError> {
    request.render(Inertia::response("Empty", ()), shell)
}
async fn custom_url(request: InertiaRequest) -> Result<Response, InertiaError> {
    request.render(
        Inertia::response("Users/Show", json!({"user":{"id":123,"name":"Ada"}}))
            .with_url("/canonical/users/123"),
        shell,
    )
}
async fn location(request: InertiaRequest) -> Result<Response, InertiaError> {
    request.location(Inertia::location("https://example.com/outside"))
}
async fn fragment_location(request: InertiaRequest) -> Result<Response, InertiaError> {
    request.location(Inertia::location("https://example.com/outside#details"))
}
async fn relative_location(request: InertiaRequest) -> Result<Response, InertiaError> {
    request.location(Inertia::location("../outside?from=axum#fragment"))
}
async fn redirect(request: InertiaRequest) -> Result<Response, InertiaError> {
    request.redirect(Inertia::redirect("/target"))
}
async fn relative_redirect(request: InertiaRequest) -> Result<Response, InertiaError> {
    request.redirect(Inertia::redirect("?next=target#fragment"))
}
async fn invalid_location(request: InertiaRequest) -> Result<Response, InertiaError> {
    request.location(Inertia::location("bad location"))
}
async fn invalid_redirect(request: InertiaRequest) -> Result<Response, InertiaError> {
    request.redirect(Inertia::redirect("bad location"))
}
async fn unsafe_page(request: InertiaRequest) -> Result<Response, InertiaError> {
    request.render(
        Inertia::response(
            "Unsafe",
            json!({"text":"</script><script>alert(1)</script>&\u{2028}\u{2029}"}),
        ),
        shell,
    )
}
async fn context(request: InertiaRequest) -> Result<Response, InertiaError> {
    let c = request.context();
    request.render(Inertia::response("Context", json!({"errorBag":c.error_bag(),"prefetch":c.is_prefetch(),"reload":c.is_reload(),"partialComponent":c.partial_component()})), shell)
}
async fn counter(
    request: InertiaRequest,
    axum::Extension(counter): axum::Extension<Arc<AtomicUsize>>,
) -> Result<Response, InertiaError> {
    counter.fetch_add(1, Ordering::SeqCst);
    request.render(Inertia::response("Counter", json!({"ok":true})), shell)
}

fn routes() -> Router {
    Router::new()
        .route("/users/{id}", get(users))
        .route(
            "/events",
            get(events)
                .post(events)
                .put(events)
                .patch(events)
                .delete(events),
        )
        .route("/advanced", get(advanced))
        .route("/lazy", get(lazy))
        .route("/scroll", get(scroll))
        .route("/history", get(history))
        .route("/errors", get(errors))
        .route("/empty", get(empty))
        .route("/custom-url", get(custom_url))
        .route(
            "/external",
            get(location)
                .post(location)
                .put(location)
                .patch(location)
                .delete(location),
        )
        .route("/external-fragment", get(fragment_location))
        .route("/relative-external", get(relative_location))
        .route(
            "/redirect",
            get(redirect)
                .post(redirect)
                .put(redirect)
                .patch(redirect)
                .delete(redirect),
        )
        .route("/relative-redirect", get(relative_redirect))
        .route("/invalid-location", get(invalid_location))
        .route("/invalid-redirect", get(invalid_redirect))
        .route(
            "/context",
            get(context)
                .post(context)
                .put(context)
                .patch(context)
                .delete(context),
        )
        .route(
            "/handler-counter",
            get(counter)
                .post(counter)
                .put(counter)
                .patch(counter)
                .delete(counter),
        )
        .route("/unsafe", get(unsafe_page))
        .nest(
            "/nested",
            Router::new().route(
                "/page",
                get(|request: InertiaRequest| async move {
                    request.render(Inertia::response("Nested", json!({"ok":true})), shell)
                }),
            ),
        )
}
pub fn app() -> Router {
    routes()
        .layer(Extension(SharedProps::new()))
        .layer(VersionLayer::new(VERSION))
}
pub fn app_without_version_layer() -> Router {
    routes()
}
pub fn app_with_dynamic_version(counter: Arc<AtomicUsize>) -> Router {
    routes().layer(VersionLayer::dynamic(move || {
        format!("asset-{}", counter.fetch_add(1, Ordering::SeqCst))
    }))
}
pub fn app_with_shared_props(shared: SharedProps) -> Router {
    routes()
        .layer(Extension(shared))
        .layer(VersionLayer::new(VERSION))
}
pub fn handler_counter_app(counter: Arc<AtomicUsize>) -> Router {
    routes()
        .layer(Extension(counter))
        .layer(VersionLayer::new(VERSION))
}
