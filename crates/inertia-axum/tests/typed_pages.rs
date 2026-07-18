//! Typed direct-response page integration coverage.

#![allow(dead_code)]
#![cfg(feature = "macros")]

use axum::{
    Router,
    body::{Body, to_bytes},
    http::Request,
    routing::get,
};
use inertia_axum::prelude::*;
use inertia_axum::{IntoInertiaProps, PropKey};
use serde_json::{Value, json};
use std::{convert::Infallible, marker::PhantomData};
use tower::ServiceExt;

#[derive(InertiaPage)]
#[inertia(
    component = "Todos/Index",
    rename_all = "camelCase",
    encrypt_history,
    clear_history,
    preserve_fragment
)]
struct TodosPage {
    todos: Vec<String>,
    stats: Prop<u64>,
    #[inertia(rename = "canCreate")]
    can_create: bool,
    #[inertia(skip)]
    internal: PhantomData<()>,
}

#[derive(InertiaProps)]
#[inertia(rename_all = "camelCase")]
struct DerivedProps {
    app_name: String,
}

fn assert_key<T>(_key: PropKey<T>) {}

#[tokio::test]
async fn derived_page_is_a_direct_typed_response_with_keys_and_options() {
    assert_eq!(TodosPage::COMPONENT.as_str(), "Todos/Index");
    assert_key::<Vec<String>>(TodosPage::TODOS);
    assert_key::<u64>(TodosPage::STATS);
    assert_key::<bool>(TodosPage::CAN_CREATE);
    assert_eq!(TodosPage::CAN_CREATE.name(), "canCreate");
    let _ = DerivedProps {
        app_name: "Demo".to_owned(),
    }
    .into_inertia_props();

    async fn handler() -> PendingPage {
        PendingPage::typed(TodosPage {
            todos: vec!["Ship derives".to_owned()],
            stats: defer(|| async { Ok::<_, Infallible>(7) }),
            can_create: true,
            internal: PhantomData,
        })
    }
    let app = Router::new()
        .route("/", get(handler))
        .inertia(InertiaApp::default_root().build().unwrap());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/")
                .header("x-inertia", "true")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let page: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(page["component"], "Todos/Index");
    assert_eq!(
        page["props"],
        json!({"canCreate":true,"errors":{},"todos":["Ship derives"]})
    );
    assert_eq!(page["deferredProps"], json!({"default":["stats"]}));
    assert_eq!(page["encryptHistory"], true);
    assert_eq!(page["clearHistory"], true);
    assert_eq!(page["preserveFragment"], true);
}
