use axum::{
    body::{to_bytes, Body},
    http::{Method, Request, StatusCode},
    routing::{get, post},
    Router,
};
use inertia_axum::{
    page, DynamicPage, InertiaApp, MemoryTransient, Redirect, RouterInertiaExt, TransientData,
    TransientRequest, TransientStore, X_INERTIA, X_INERTIA_VERSION,
};
use serde_json::{json, Value};
use std::{
    convert::Infallible,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use tower::ServiceExt;

async fn body(response: axum::response::Response) -> String {
    String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap()
}
async fn page(response: axum::response::Response) -> Value {
    serde_json::from_str(&body(response).await).unwrap()
}
fn request(method: Method, uri: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(X_INERTIA, "true")
        .body(Body::empty())
        .unwrap()
}

fn flash_app(store: MemoryTransient) -> Router {
    async fn create() -> Redirect {
        Redirect::to("/target")
            .flash("toast", json!({"kind":"success","message":"Created"}))
            .flash("newProjectId", 42)
    }
    async fn target() -> DynamicPage {
        page!("Target", { value: 1 })
    }
    async fn attached() -> DynamicPage {
        page!("Attached", { value: 1 }).flash("highlight", 7)
    }
    Router::new()
        .route("/create", post(create))
        .route("/target", get(target))
        .route("/attached", get(attached))
        .inertia(InertiaApp::default_root().transient(store).build().unwrap())
}

#[tokio::test]
async fn redirect_flash_has_multiple_keys_and_is_consumed_once() {
    let app = flash_app(MemoryTransient::new());
    let redirect = app
        .clone()
        .oneshot(request(Method::POST, "/create"))
        .await
        .unwrap();
    assert_eq!(redirect.status(), StatusCode::SEE_OTHER);
    let first = page(
        app.clone()
            .oneshot(request(Method::GET, "/target"))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(first["flash"]["toast"]["message"], "Created");
    assert_eq!(first["flash"]["newProjectId"], 42);
    assert!(first["props"].get("toast").is_none());
    let second = page(app.oneshot(request(Method::GET, "/target")).await.unwrap()).await;
    assert!(second.get("flash").is_none());
}

#[tokio::test]
async fn page_attached_flash_is_current_response_only() {
    let app = flash_app(MemoryTransient::new());
    let attached = page(
        app.clone()
            .oneshot(request(Method::GET, "/attached"))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(attached["flash"]["highlight"], 7);
    let next = page(app.oneshot(request(Method::GET, "/target")).await.unwrap()).await;
    assert!(next.get("flash").is_none());
}

#[tokio::test]
async fn flash_without_a_store_has_an_actionable_error() {
    async fn handler() -> Redirect {
        Redirect::to("/").flash("toast", "saved")
    }
    let app = Router::new()
        .route("/", post(handler))
        .inertia(InertiaApp::default_root().build().unwrap());
    let response = app.oneshot(request(Method::POST, "/")).await.unwrap();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert!(body(response)
        .await
        .contains("configure InertiaAppBuilder::transient"));
}

#[tokio::test]
async fn stale_version_conflict_reflashes_without_running_handler() {
    let calls = Arc::new(AtomicUsize::new(0));
    let target_calls = calls.clone();
    let app = Router::new()
        .route(
            "/create",
            post(|| async { Redirect::to("/target").flash("toast", "pending") }),
        )
        .route(
            "/target",
            get(move || {
                let calls = target_calls.clone();
                async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    page!("Target", { value: 1 })
                }
            }),
        )
        .inertia(
            InertiaApp::default_root()
                .version("v1")
                .transient(MemoryTransient::new())
                .build()
                .unwrap(),
        );
    app.clone()
        .oneshot(request(Method::POST, "/create"))
        .await
        .unwrap();
    let stale = Request::builder()
        .uri("/target")
        .header(X_INERTIA, "true")
        .header(X_INERTIA_VERSION, "stale")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app.clone().oneshot(stale).await.unwrap().status(),
        StatusCode::CONFLICT
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let fresh = Request::builder()
        .uri("/target")
        .header(X_INERTIA, "true")
        .header(X_INERTIA_VERSION, "v1")
        .body(Body::empty())
        .unwrap();
    let page = page(app.oneshot(fresh).await.unwrap()).await;
    assert_eq!(page["flash"]["toast"], "pending");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[derive(Clone)]
struct CountingStore {
    loads: Arc<AtomicUsize>,
    commits: Arc<AtomicUsize>,
}
impl TransientStore for CountingStore {
    type Error = Infallible;
    async fn load(&self, _request: TransientRequest<'_>) -> Result<TransientData, Self::Error> {
        self.loads.fetch_add(1, Ordering::SeqCst);
        Ok(TransientData::default())
    }
    async fn commit(
        &self,
        _response: &mut axum::response::Response,
        _data: TransientData,
    ) -> Result<(), Self::Error> {
        self.commits.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[tokio::test]
async fn unrelated_ordinary_routes_do_not_touch_transient_storage() {
    let loads = Arc::new(AtomicUsize::new(0));
    let commits = Arc::new(AtomicUsize::new(0));
    let store = CountingStore {
        loads: loads.clone(),
        commits: commits.clone(),
    };
    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .inertia(InertiaApp::default_root().transient(store).build().unwrap());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(body(response).await, "ok");
    assert_eq!(loads.load(Ordering::SeqCst), 0);
    assert_eq!(commits.load(Ordering::SeqCst), 0);
}

#[cfg(feature = "cookies")]
#[tokio::test]
async fn encrypted_cookie_commit_is_secure_and_opaque() {
    use axum::http::header::SET_COOKIE;
    use inertia_axum::CookieTransient;
    let app = Router::new()
        .route(
            "/",
            post(|| async { Redirect::to("/").flash("secret", "not-visible") }),
        )
        .inertia(
            InertiaApp::default_root()
                .transient(CookieTransient::encrypted([7_u8; 32]))
                .build()
                .unwrap(),
        );
    let response = app.oneshot(request(Method::POST, "/")).await.unwrap();
    let cookie = response
        .headers()
        .get(SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(cookie.contains("Secure"));
    assert!(cookie.contains("HttpOnly"));
    assert!(cookie.contains("SameSite=Lax"));
    assert!(!cookie.contains("not-visible"));
}
