use super::support::*;
use axum::{
    body::Body,
    http::{Method, Request},
};
use inertia_axum::{
    RequestContext, X_INERTIA, X_INERTIA_ERROR_BAG, X_INERTIA_EXCEPT_ONCE_PROPS,
    X_INERTIA_INFINITE_SCROLL_MERGE_INTENT, X_INERTIA_PARTIAL_COMPONENT, X_INERTIA_PARTIAL_DATA,
    X_INERTIA_PARTIAL_EXCEPT, X_INERTIA_RESET, X_INERTIA_VERSION,
};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ContextSnapshot<'a> {
    is_inertia: bool,
    version: Option<&'a str>,
    partial_component: Option<&'a str>,
    partial_data: Vec<String>,
    partial_except: Vec<String>,
    reset: Vec<String>,
    error_bag: Option<&'a str>,
    merge_intent: Option<&'a str>,
    except_once_props: Vec<String>,
    prefetch: bool,
    reload: bool,
}
#[tokio::test]
async fn request_context_parses_all_supported_headers() {
    let headers = [
        (X_INERTIA, "true"),
        (X_INERTIA_VERSION, VERSION),
        (X_INERTIA_PARTIAL_COMPONENT, "Events/Index"),
        (X_INERTIA_PARTIAL_DATA, "events, filters"),
        (X_INERTIA_PARTIAL_EXCEPT, "categories"),
        (X_INERTIA_RESET, "events"),
        (X_INERTIA_ERROR_BAG, "createEvent"),
        (X_INERTIA_INFINITE_SCROLL_MERGE_INTENT, "prepend"),
        (X_INERTIA_EXCEPT_ONCE_PROPS, "plans, permissions"),
        ("Purpose", "PREFETCH"),
        ("Cache-Control", "max-age=0, NO-CACHE"),
    ];
    let context = RequestContext::from_header_fn(|name| {
        headers
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| *value)
    });
    assert!(context.is_inertia() && context.is_prefetch() && context.is_reload());
    assert_eq!(context.partial_data(), ["events", "filters"]);
    insta::assert_json_snapshot!(
        "request_context_all_headers",
        ContextSnapshot {
            is_inertia: context.is_inertia(),
            version: context.version(),
            partial_component: context.partial_component(),
            partial_data: context.partial_data(),
            partial_except: context.partial_except(),
            reset: context.reset(),
            error_bag: context.error_bag(),
            merge_intent: context.infinite_scroll_merge_intent(),
            except_once_props: context.except_once_props(),
            prefetch: context.is_prefetch(),
            reload: context.is_reload()
        }
    );
}
#[test]
fn header_lists_trim_whitespace_and_ignore_empty_entries() {
    let c = RequestContext::from_header_fn(|n| {
        (n == X_INERTIA_PARTIAL_DATA).then_some("events, filters, ,categories,")
    });
    assert_eq!(c.partial_data(), ["events", "filters", "categories"]);
}
#[test]
fn prefetch_and_reload_are_case_insensitive() {
    let c = RequestContext::from_header_fn(|n| match n {
        "Purpose" => Some("PREFETCH"),
        "Cache-Control" => Some("max-age=0, NO-CACHE"),
        _ => None,
    });
    assert!(c.is_prefetch() && c.is_reload());
}
#[tokio::test]
async fn axum_header_lookup_is_case_insensitive_and_writes_discard_partial_state() {
    let request = Request::builder()
        .method(Method::POST)
        .uri("/context")
        .header("x-inertia", "true")
        .header("x-inertia-partial-component", "Events/Index")
        .header("x-inertia-partial-data", "events")
        .header("x-inertia-error-bag", "bag")
        .header("purpose", "prefetch")
        .body(Body::empty())
        .unwrap();
    let response = call(app(), request).await;
    let p = &response.page().unwrap()["props"];
    assert_eq!(p["errorBag"], "bag");
    assert_eq!(p["prefetch"], true);
    assert_eq!(p["partialComponent"], "Events/Index");
    insta::assert_json_snapshot!("write_context_discards_partial_headers", response);
}
