//! Typed shared-provider integration coverage.
#![allow(dead_code)]
#![cfg(feature = "macros")]

use axum::{
    Extension, Router,
    body::{Body, to_bytes},
    http::{Method, Request},
    routing::get,
};
use inertia_axum::prelude::*;
use serde::{Serialize, Serializer};
use serde_json::{Value, json};
use std::{
    convert::Infallible,
    io,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
use tower::ServiceExt;

#[derive(Clone)]
struct CurrentUser(&'static str);

#[derive(InertiaProps)]
#[inertia(rename_all = "camelCase")]
struct AppShared {
    app_name: String,
    #[inertia(rename = "auth.user")]
    auth_user: Prop<String>,
    countries: Prop<Vec<String>>,
    audit: Prop<u32>,
}

#[derive(Clone)]
struct AppShare {
    auth_constructed: Arc<AtomicUsize>,
    countries_constructed: Arc<AtomicUsize>,
    audit_constructed: Arc<AtomicUsize>,
    contexts: Arc<Mutex<Vec<String>>>,
}

impl Share for AppShare {
    type Props = AppShared;
    type Error = Infallible;

    fn share(&self, context: ShareContext<'_>) -> Result<Self::Props, Self::Error> {
        self.contexts.lock().unwrap().push(format!(
            "{}:{}:{}:{}:{}",
            context.method(),
            context.uri(),
            context
                .headers()
                .get("x-test")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("none"),
            context.visit().is_inertia(),
            context
                .extension::<CurrentUser>()
                .map_or("none", |user| user.0),
        ));
        let auth = self.auth_constructed.clone();
        let countries = self.countries_constructed.clone();
        let audit = self.audit_constructed.clone();
        Ok(AppShared {
            app_name: "Typed App".to_owned(),
            auth_user: lazy(move || {
                auth.fetch_add(1, Ordering::SeqCst);
                async { Ok::<_, io::Error>("Ada".to_owned()) }
            }),
            countries: once(move || {
                countries.fetch_add(1, Ordering::SeqCst);
                async { Ok::<_, io::Error>(vec!["CA".to_owned()]) }
            })
            .key("countries:v1"),
            audit: optional(move || {
                audit.fetch_add(1, Ordering::SeqCst);
                async { Ok::<_, io::Error>(9) }
            }),
        })
    }
}

#[derive(InertiaPage)]
#[inertia(component = "Home")]
struct Home {
    title: String,
}

#[derive(InertiaPage)]
#[inertia(component = "Owned")]
struct RouteOwnsAuth {
    title: String,
    auth: Prop<String>,
}

type SharedFixture = (
    AppShare,
    Arc<AtomicUsize>,
    Arc<AtomicUsize>,
    Arc<AtomicUsize>,
    Arc<Mutex<Vec<String>>>,
);

fn counters() -> SharedFixture {
    let auth = Arc::new(AtomicUsize::new(0));
    let countries = Arc::new(AtomicUsize::new(0));
    let audit = Arc::new(AtomicUsize::new(0));
    let contexts = Arc::new(Mutex::new(Vec::new()));
    (
        AppShare {
            auth_constructed: auth.clone(),
            countries_constructed: countries.clone(),
            audit_constructed: audit.clone(),
            contexts: contexts.clone(),
        },
        auth,
        countries,
        audit,
        contexts,
    )
}

async fn page(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
}

#[tokio::test]
async fn typed_shared_data_uses_borrowed_context_and_dotted_deduplicated_roots() {
    let (share, auth, countries, audit, contexts) = counters();
    let inertia = InertiaApp::default_root().share(share).build().unwrap();
    let app = Router::new()
        .route(
            "/",
            get(|| async {
                PendingPage::typed(Home {
                    title: "Hi".to_owned(),
                })
            }),
        )
        .inertia(inertia)
        .layer(Extension(CurrentUser("Grace")));
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/?tab=all")
                .header("x-inertia", "true")
                .header("x-test", "seen")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let page = page(response).await;
    assert_eq!(page["props"]["appName"], "Typed App");
    assert_eq!(page["props"]["auth"]["user"], "Ada");
    assert_eq!(page["props"]["countries"], json!(["CA"]));
    assert_eq!(page["sharedProps"], json!(["appName", "auth", "countries"]));
    assert_eq!(page["onceProps"]["countries:v1"]["prop"], "countries");
    assert_eq!(auth.load(Ordering::SeqCst), 1);
    assert_eq!(countries.load(Ordering::SeqCst), 1);
    assert_eq!(audit.load(Ordering::SeqCst), 0);
    assert_eq!(contexts.lock().unwrap()[0], "GET:/?tab=all:seen:true:Grace");
}

#[tokio::test]
async fn declared_route_root_blocks_shared_future_even_when_route_value_is_omitted() {
    let (share, auth, _, _, _) = counters();
    let app = Router::new()
        .route(
            "/",
            get(|| async {
                PendingPage::typed(RouteOwnsAuth {
                    title: "Owned".to_owned(),
                    auth: optional(|| async { Ok::<_, io::Error>("route".to_owned()) }),
                })
            }),
        )
        .inertia(InertiaApp::default_root().share(share).build().unwrap());
    let page = page(
        app.oneshot(
            Request::builder()
                .uri("/")
                .header("x-inertia", "true")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap(),
    )
    .await;
    assert!(page["props"].get("auth").is_none());
    assert_eq!(auth.load(Ordering::SeqCst), 0);
    assert!(
        !page["sharedProps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|root| root == "auth")
    );
}

#[tokio::test]
async fn optional_and_once_shared_props_use_common_selection_engine() {
    let (share, _, countries, audit, _) = counters();
    let app = Router::new()
        .route(
            "/",
            get(|| async {
                PendingPage::typed(Home {
                    title: "Hi".to_owned(),
                })
            }),
        )
        .inertia(InertiaApp::default_root().share(share).build().unwrap());
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/")
                .header("x-inertia", "true")
                .header("x-inertia-partial-component", "Home")
                .header("x-inertia-partial-data", "audit")
                .header("x-inertia-except-once-props", "countries:v1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let page = page(response).await;
    assert_eq!(page["props"]["appName"], "Typed App");
    assert_eq!(page["props"]["audit"], 9);
    assert!(page["props"].get("countries").is_none());
    assert_eq!(countries.load(Ordering::SeqCst), 0);
    assert_eq!(audit.load(Ordering::SeqCst), 1);
}

struct Counted(Arc<AtomicUsize>);
impl Serialize for Counted {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.fetch_add(1, Ordering::SeqCst);
        serializer.serialize_str("counted")
    }
}

#[derive(InertiaProps)]
#[inertia(typegen(skip))]
struct HealthShared {
    value: Counted,
    optional: Prop<u32>,
}

#[derive(Clone)]
struct HealthShare {
    serialized: Arc<AtomicUsize>,
    constructed: Arc<AtomicUsize>,
}
impl Share for HealthShare {
    type Props = HealthShared;
    type Error = Infallible;
    fn share(&self, _context: ShareContext<'_>) -> Result<Self::Props, Self::Error> {
        let constructed = self.constructed.clone();
        Ok(HealthShared {
            value: Counted(self.serialized.clone()),
            optional: optional(move || {
                constructed.fetch_add(1, Ordering::SeqCst);
                async { Ok::<_, io::Error>(1) }
            }),
        })
    }
}

#[tokio::test]
async fn ordinary_responses_do_not_serialize_or_construct_shared_prop_futures() {
    let serialized = Arc::new(AtomicUsize::new(0));
    let constructed = Arc::new(AtomicUsize::new(0));
    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .inertia(
            InertiaApp::default_root()
                .share(HealthShare {
                    serialized: serialized.clone(),
                    constructed: constructed.clone(),
                })
                .build()
                .unwrap(),
        );
    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .to_vec()
        )
        .unwrap(),
        "ok"
    );
    assert_eq!(serialized.load(Ordering::SeqCst), 0);
    assert_eq!(constructed.load(Ordering::SeqCst), 0);
}
