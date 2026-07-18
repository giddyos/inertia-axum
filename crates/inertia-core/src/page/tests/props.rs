//! Page construction and prop-selection assertions.

//! Page construction and protocol behavior tests.

use serde_json::Value;

use crate::*;
use serde_json::json;
use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

fn request_context_from(headers: &[(&str, &str)]) -> RequestContext {
    let headers = headers.iter().copied().collect::<HashMap<_, _>>();

    RequestContext::from_header_fn(|name| headers.get(name).copied())
}

#[test]
fn request_context_parses_inertia_headers() {
    let context = request_context_from(&[
        (X_INERTIA, "true"),
        (X_INERTIA_VERSION, "abc"),
        (X_INERTIA_PARTIAL_COMPONENT, "Users/Index"),
        (X_INERTIA_PARTIAL_DATA, "users, stats"),
        (X_INERTIA_RESET, "users"),
        (X_INERTIA_ERROR_BAG, "createUser"),
        (X_INERTIA_INFINITE_SCROLL_MERGE_INTENT, "append"),
        (X_INERTIA_EXCEPT_ONCE_PROPS, "plans,features"),
        (PURPOSE, "prefetch"),
        (CACHE_CONTROL, "max-age=0, no-cache"),
    ]);

    assert!(context.is_inertia());
    assert_eq!(context.version(), Some("abc"));
    assert_eq!(context.partial_component(), Some("Users/Index"));
    assert_eq!(context.partial_data(), ["users", "stats"]);
    assert_eq!(context.reset(), ["users"]);
    assert_eq!(context.error_bag(), Some("createUser"));
    assert_eq!(context.infinite_scroll_merge_intent(), Some("append"));
    assert_eq!(context.except_once_props(), ["plans", "features"]);
    assert!(context.is_prefetch());
    assert!(context.is_reload());
}

#[test]
fn page_serializes_v3_metadata() {
    let page = Page::from_parts(
        "Feed/Index",
        json!({ "errors": {}, "posts": [{ "id": 1 }] }),
        "/feed",
        Some("version-1".into()),
        PageMetadata::new()
            .encrypt_history()
            .clear_history()
            .preserve_fragment()
            .merge("posts")
            .prepend("notifications")
            .deep_merge("conversations")
            .match_on("posts.id")
            .scroll("posts", ScrollProps::new("page", 1).next_page(2))
            .defer("analytics")
            .rescue("analytics")
            .share("auth")
            .once("plans"),
    );

    let value = serde_json::to_value(page).unwrap();

    assert_eq!(value["component"], "Feed/Index");
    assert_eq!(value["version"], "version-1");
    assert_eq!(value["encryptHistory"], true);
    assert_eq!(value["clearHistory"], true);
    assert_eq!(value["preserveFragment"], true);
    assert_eq!(value["mergeProps"], json!(["posts", "posts.data"]));
    assert_eq!(value["prependProps"], json!(["notifications"]));
    assert_eq!(value["deepMergeProps"], json!(["conversations"]));
    assert_eq!(value["matchPropsOn"], json!(["posts.id"]));
    assert_eq!(value["scrollProps"]["posts"]["pageName"], "page");
    assert_eq!(value["scrollProps"]["posts"]["nextPage"], 2);
    assert_eq!(value["deferredProps"], json!({ "default": ["analytics"] }));
    assert_eq!(value["rescuedProps"], json!(["analytics"]));
    assert_eq!(value["sharedProps"], json!(["auth"]));
    assert_eq!(
        value["onceProps"]["plans"],
        json!({ "prop": "plans", "expiresAt": null })
    );
}

#[test]
fn route_local_shared_values_merge_before_global_values() {
    let page = Inertia::response("Dashboard", serde_json::json!({}))
        .shared_value("auth.user", serde_json::json!({"name": "Route"}))
        .into_page("/dashboard", None, &RequestContext::default())
        .unwrap()
        .with_shared_props([("auth.user", serde_json::json!({"name": "Global"}))]);

    assert_eq!(page.props()["auth"]["user"]["name"], "Route");
}

#[test]
fn partial_data_filters_matching_component_props() {
    let context = request_context_from(&[
        (X_INERTIA, "true"),
        (X_INERTIA_PARTIAL_COMPONENT, "Events"),
        (X_INERTIA_PARTIAL_DATA, "events"),
    ]);
    let mut props = json!({
        "auth": { "name": "Ada" },
        "events": [1, 2],
        "categories": ["meetups"]
    });

    context.filter_props("Events", &mut props, &PageMetadata::new().always("auth"));

    assert_eq!(
        props,
        json!({
            "errors": {},
            "auth": { "name": "Ada" },
            "events": [1, 2]
        })
    );
}

#[test]
fn partial_except_takes_precedence_over_partial_data() {
    let context = request_context_from(&[
        (X_INERTIA, "true"),
        (X_INERTIA_PARTIAL_COMPONENT, "Events"),
        (X_INERTIA_PARTIAL_DATA, "events"),
        (X_INERTIA_PARTIAL_EXCEPT, "categories"),
    ]);
    let mut props = json!({
        "auth": { "name": "Ada" },
        "events": [1, 2],
        "categories": ["meetups"]
    });

    context.filter_props("Events", &mut props, &PageMetadata::new());

    assert_eq!(
        props,
        json!({
            "errors": {},
            "auth": { "name": "Ada" },
            "events": [1, 2]
        })
    );
}

#[test]
fn partial_except_without_partial_data_excludes_listed_props() {
    let context = request_context_from(&[
        (X_INERTIA, "true"),
        (X_INERTIA_PARTIAL_COMPONENT, "Events"),
        (X_INERTIA_PARTIAL_EXCEPT, "categories"),
    ]);
    let mut props = json!({
        "events": [1, 2],
        "categories": ["meetups"],
        "filters": { "open": true }
    });

    context.filter_props("Events", &mut props, &PageMetadata::new());

    assert_eq!(
        props,
        json!({
            "errors": {},
            "events": [1, 2],
            "filters": { "open": true }
        })
    );
}

#[test]
fn partial_headers_are_ignored_for_different_components() {
    let context = request_context_from(&[
        (X_INERTIA, "true"),
        (X_INERTIA_PARTIAL_COMPONENT, "Events"),
        (X_INERTIA_PARTIAL_DATA, "events"),
    ]);
    let mut props = json!({
        "auth": { "name": "Ada" },
        "events": [1, 2]
    });

    context.filter_props("Dashboard", &mut props, &PageMetadata::new());

    assert_eq!(
        props,
        json!({
            "errors": {},
            "auth": { "name": "Ada" },
            "events": [1, 2]
        })
    );
}

#[test]
fn deferred_and_once_props_are_omitted_until_explicitly_requested() {
    let context =
        request_context_from(&[(X_INERTIA, "true"), (X_INERTIA_EXCEPT_ONCE_PROPS, "plans")]);
    let mut props = json!({
        "analytics": { "views": 10 },
        "plans": ["basic"],
        "user": { "name": "Ada" }
    });
    let metadata = PageMetadata::new().defer("analytics").once("plans");

    context.filter_props("Dashboard", &mut props, &metadata);

    assert_eq!(
        props,
        json!({
            "errors": {},
            "user": { "name": "Ada" }
        })
    );

    let context = request_context_from(&[
        (X_INERTIA, "true"),
        (X_INERTIA_PARTIAL_COMPONENT, "Dashboard"),
        (X_INERTIA_PARTIAL_DATA, "analytics,plans"),
        (X_INERTIA_EXCEPT_ONCE_PROPS, "plans"),
    ]);
    let mut props = json!({
        "analytics": { "views": 10 },
        "plans": ["basic"],
        "user": { "name": "Ada" }
    });

    context.filter_props("Dashboard", &mut props, &metadata);

    assert_eq!(
        props,
        json!({
            "analytics": { "views": 10 },
            "errors": {},
            "plans": ["basic"]
        })
    );
}

#[test]
fn request_reset_filters_merge_and_scroll_metadata() {
    let request = request_context_from(&[
        (X_INERTIA, "true"),
        (X_INERTIA_PARTIAL_COMPONENT, "Feed"),
        (X_INERTIA_PARTIAL_DATA, "posts"),
        (X_INERTIA_RESET, "posts"),
    ]);
    let response = Inertia::page("Feed")
        .scroll("posts", ScrollProps::new("page", 1).next_page(2))
        .match_on("posts.data.id")
        .props(json!({ "posts": { "data": [1, 2] } }))
        .into_page("/feed", Some("version-1".into()), &request)
        .unwrap();
    let value = serde_json::to_value(response).unwrap();

    assert_eq!(value["props"]["posts"]["data"], json!([1, 2]));
    assert!(value.get("mergeProps").is_none());
    assert!(value.get("matchPropsOn").is_none());
    assert!(value.get("scrollProps").is_none());
}

#[test]
fn reset_and_scroll_intent_are_ignored_when_partial_component_differs() {
    let request = request_context_from(&[
        (X_INERTIA, "true"),
        (X_INERTIA_PARTIAL_COMPONENT, "Other"),
        (X_INERTIA_PARTIAL_DATA, "posts"),
        (X_INERTIA_RESET, "posts"),
        (X_INERTIA_INFINITE_SCROLL_MERGE_INTENT, "prepend"),
    ]);
    let response = Inertia::page("Feed")
        .scroll("posts", ScrollProps::new("page", 1).next_page(2))
        .props(json!({ "posts": { "data": [1, 2] } }))
        .into_page("/feed", Some("version-1".into()), &request)
        .unwrap();
    let value = serde_json::to_value(response).unwrap();

    assert_eq!(value["props"]["posts"]["data"], json!([1, 2]));
    assert_eq!(value["mergeProps"], json!(["posts.data"]));
    assert_eq!(value["scrollProps"]["posts"]["nextPage"], 2);
    assert!(value.get("prependProps").is_none());
}

#[test]
fn infinite_scroll_merge_intent_can_prepend_scroll_props() {
    let request = request_context_from(&[
        (X_INERTIA, "true"),
        (X_INERTIA_PARTIAL_COMPONENT, "Feed"),
        (X_INERTIA_PARTIAL_DATA, "posts"),
        (X_INERTIA_INFINITE_SCROLL_MERGE_INTENT, "prepend"),
    ]);
    let response = Inertia::page("Feed")
        .scroll("posts", ScrollProps::new("page", 1).next_page(2))
        .props(json!({ "posts": { "data": [1, 2] } }))
        .into_page("/feed", Some("version-1".into()), &request)
        .unwrap();
    let value = serde_json::to_value(response).unwrap();

    assert_eq!(value["prependProps"], json!(["posts.data"]));
    assert!(value.get("mergeProps").is_none());
    assert_eq!(value["scrollProps"]["posts"]["nextPage"], 2);
}

#[test]
fn once_with_custom_key_omits_loaded_prop_until_requested() {
    let request = request_context_from(&[
        (X_INERTIA, "true"),
        (X_INERTIA_EXCEPT_ONCE_PROPS, "billing"),
    ]);
    let response = Inertia::response(
        "Billing",
        json!({
            "current_plan": "basic",
            "plans": ["basic", "pro"]
        }),
    )
    .once_with_key("billing", OnceProp::new("plans").expires_at(123))
    .into_page("/billing", Some("version-1".into()), &request)
    .unwrap();
    let value = serde_json::to_value(response).unwrap();

    assert!(value["props"].get("plans").is_none());
    assert_eq!(
        value["onceProps"]["billing"],
        json!({ "prop": "plans", "expiresAt": 123 })
    );
}

#[test]
fn lazy_props_are_only_resolved_when_included() {
    let request = request_context_from(&[]);
    let calls = Rc::new(Cell::new(0));
    let response = Inertia::response(
        "Dashboard",
        InertiaProps::new()
            .value("user", json!({ "name": "Ada" }))
            .lazy("stats", {
                let calls = Rc::clone(&calls);
                move || {
                    calls.set(calls.get() + 1);
                    json!({ "views": 10 })
                }
            }),
    )
    .into_page("/dashboard", Some("version-1".into()), &request)
    .unwrap();
    let value = serde_json::to_value(response).unwrap();

    assert_eq!(calls.get(), 1);
    assert_eq!(value["props"]["stats"]["views"], 10);

    let request = request_context_from(&[
        (X_INERTIA, "true"),
        (X_INERTIA_PARTIAL_COMPONENT, "Dashboard"),
        (X_INERTIA_PARTIAL_DATA, "user"),
    ]);
    let calls = Rc::new(Cell::new(0));
    let response = Inertia::response(
        "Dashboard",
        InertiaProps::new()
            .value("user", json!({ "name": "Ada" }))
            .lazy("stats", {
                let calls = Rc::clone(&calls);
                move || {
                    calls.set(calls.get() + 1);
                    json!({ "views": 10 })
                }
            }),
    )
    .into_page("/dashboard", Some("version-1".into()), &request)
    .unwrap();
    let value = serde_json::to_value(response).unwrap();

    assert_eq!(calls.get(), 0);
    assert_eq!(value["props"]["user"]["name"], "Ada");
    assert!(value["props"].get("stats").is_none());
}

#[test]
fn lazy_props_can_borrow_values_for_immediate_rendering() {
    let request = request_context_from(&[]);
    let name = String::from("Ada");
    let response = Inertia::response(
        "Profile",
        ScopedInertiaProps::new()
            .value("name", &name)
            .lazy("upperName", || name.to_uppercase()),
    )
    .into_page("/profile", Some("version-1".into()), &request)
    .unwrap();
    let value = serde_json::to_value(response).unwrap();

    assert_eq!(value["props"]["name"], "Ada");
    assert_eq!(value["props"]["upperName"], "ADA");
}

#[test]
fn optional_props_resolve_only_when_explicitly_requested() {
    let request = request_context_from(&[]);
    let calls = Rc::new(Cell::new(0));
    let response = Inertia::response(
        "Dashboard",
        InertiaProps::new().optional("audit", {
            let calls = Rc::clone(&calls);
            move || {
                calls.set(calls.get() + 1);
                json!(["created"])
            }
        }),
    )
    .into_page("/dashboard", Some("version-1".into()), &request)
    .unwrap();
    let value = serde_json::to_value(response).unwrap();

    assert_eq!(calls.get(), 0);
    assert!(value["props"].get("audit").is_none());

    let request = request_context_from(&[
        (X_INERTIA, "true"),
        (X_INERTIA_PARTIAL_COMPONENT, "Dashboard"),
        (X_INERTIA_PARTIAL_DATA, "audit"),
    ]);
    let calls = Rc::new(Cell::new(0));
    let response = Inertia::response(
        "Dashboard",
        InertiaProps::new().optional("audit", {
            let calls = Rc::clone(&calls);
            move || {
                calls.set(calls.get() + 1);
                json!(["created"])
            }
        }),
    )
    .into_page("/dashboard", Some("version-1".into()), &request)
    .unwrap();
    let value = serde_json::to_value(response).unwrap();

    assert_eq!(calls.get(), 1);
    assert_eq!(value["props"]["audit"], json!(["created"]));
}

#[test]
fn optional_props_respect_partial_except_precedence() {
    let request = request_context_from(&[
        (X_INERTIA, "true"),
        (X_INERTIA_PARTIAL_COMPONENT, "Dashboard"),
        (X_INERTIA_PARTIAL_DATA, "audit"),
        (X_INERTIA_PARTIAL_EXCEPT, "audit"),
    ]);
    let calls = Rc::new(Cell::new(0));
    let response = Inertia::response(
        "Dashboard",
        InertiaProps::new().optional("audit", {
            let calls = Rc::clone(&calls);
            move || {
                calls.set(calls.get() + 1);
                json!(["created"])
            }
        }),
    )
    .into_page("/dashboard", Some("version-1".into()), &request)
    .unwrap();
    let value = serde_json::to_value(response).unwrap();

    assert_eq!(calls.get(), 0);
    assert!(value["props"].get("audit").is_none());
}

#[test]
fn lazy_errors_are_preserved_during_partial_reloads() {
    let request = request_context_from(&[
        (X_INERTIA, "true"),
        (X_INERTIA_PARTIAL_COMPONENT, "Form"),
        (X_INERTIA_PARTIAL_DATA, "user"),
    ]);
    let calls = Rc::new(Cell::new(0));
    let response = Inertia::response(
        "Form",
        InertiaProps::new()
            .value("user", json!({ "name": "Ada" }))
            .lazy("errors", {
                let calls = Rc::clone(&calls);
                move || {
                    calls.set(calls.get() + 1);
                    json!({ "name": "Required" })
                }
            })
            .lazy("stats", || 10),
    )
    .into_page("/form", Some("version-1".into()), &request)
    .unwrap();
    let value = serde_json::to_value(response).unwrap();

    assert_eq!(calls.get(), 1);
    assert_eq!(value["props"]["user"]["name"], "Ada");
    assert_eq!(value["props"]["errors"]["name"], "Required");
    assert!(value["props"].get("stats").is_none());
}

#[test]
fn deferred_props_emit_metadata_and_resolve_only_when_requested() {
    let request = request_context_from(&[]);
    let calls = Rc::new(Cell::new(0));
    let response = Inertia::response(
        "Dashboard",
        InertiaProps::new().defer_group("metrics", "analytics", {
            let calls = Rc::clone(&calls);
            move || {
                calls.set(calls.get() + 1);
                json!({ "views": 10 })
            }
        }),
    )
    .into_page("/dashboard", Some("version-1".into()), &request)
    .unwrap();
    let value = serde_json::to_value(response).unwrap();

    assert_eq!(calls.get(), 0);
    assert!(value["props"].get("analytics").is_none());
    assert_eq!(value["deferredProps"], json!({ "metrics": ["analytics"] }));

    let request = request_context_from(&[
        (X_INERTIA, "true"),
        (X_INERTIA_PARTIAL_COMPONENT, "Dashboard"),
        (X_INERTIA_PARTIAL_DATA, "analytics"),
    ]);
    let calls = Rc::new(Cell::new(0));
    let response = Inertia::response(
        "Dashboard",
        InertiaProps::new().defer_group("metrics", "analytics", {
            let calls = Rc::clone(&calls);
            move || {
                calls.set(calls.get() + 1);
                json!({ "views": 10 })
            }
        }),
    )
    .into_page("/dashboard", Some("version-1".into()), &request)
    .unwrap();
    let value = serde_json::to_value(response).unwrap();

    assert_eq!(calls.get(), 1);
    assert_eq!(value["props"]["analytics"]["views"], 10);
    assert!(value.get("deferredProps").is_none());
}

#[test]
fn deferred_once_props_already_loaded_by_client_are_not_advertised() {
    let request =
        request_context_from(&[(X_INERTIA, "true"), (X_INERTIA_EXCEPT_ONCE_PROPS, "stats")]);
    let calls = Rc::new(Cell::new(0));
    let response = Inertia::response(
        "Dashboard",
        InertiaProps::new().defer_once("stats", {
            let calls = Rc::clone(&calls);
            move || {
                calls.set(calls.get() + 1);
                10
            }
        }),
    )
    .into_page("/dashboard", Some("version-1".into()), &request)
    .unwrap();
    let value = serde_json::to_value(response).unwrap();

    assert_eq!(calls.get(), 0);
    assert!(value["props"].get("stats").is_none());
    assert!(value.get("deferredProps").is_none());
    assert_eq!(
        value["onceProps"]["stats"],
        json!({ "prop": "stats", "expiresAt": null })
    );
}

#[test]
fn always_lazy_props_survive_partial_reload_filtering() {
    let request = request_context_from(&[
        (X_INERTIA, "true"),
        (X_INERTIA_PARTIAL_COMPONENT, "Dashboard"),
        (X_INERTIA_PARTIAL_DATA, "users"),
    ]);
    let calls = Rc::new(Cell::new(0));
    let response = Inertia::response(
        "Dashboard",
        InertiaProps::new()
            .value("users", json!(["Ada"]))
            .always("auth", {
                let calls = Rc::clone(&calls);
                move || {
                    calls.set(calls.get() + 1);
                    json!({ "user": { "name": "Ada" } })
                }
            }),
    )
    .into_page("/dashboard", Some("version-1".into()), &request)
    .unwrap();
    let value = serde_json::to_value(response).unwrap();

    assert_eq!(calls.get(), 1);
    assert_eq!(value["props"]["users"], json!(["Ada"]));
    assert_eq!(value["props"]["auth"]["user"]["name"], "Ada");
}

#[test]
fn once_lazy_props_are_not_resolved_when_client_already_has_them() {
    let request =
        request_context_from(&[(X_INERTIA, "true"), (X_INERTIA_EXCEPT_ONCE_PROPS, "plans")]);
    let calls = Rc::new(Cell::new(0));
    let response = Inertia::response(
        "Billing",
        InertiaProps::new().once("plans", {
            let calls = Rc::clone(&calls);
            move || {
                calls.set(calls.get() + 1);
                json!(["basic"])
            }
        }),
    )
    .into_page("/billing", Some("version-1".into()), &request)
    .unwrap();
    let value = serde_json::to_value(response).unwrap();

    assert_eq!(calls.get(), 0);
    assert!(value["props"].get("plans").is_none());
    assert_eq!(
        value["onceProps"]["plans"],
        json!({ "prop": "plans", "expiresAt": null })
    );

    let request = request_context_from(&[
        (X_INERTIA, "true"),
        (X_INERTIA_PARTIAL_COMPONENT, "Billing"),
        (X_INERTIA_PARTIAL_DATA, "plans"),
        (X_INERTIA_EXCEPT_ONCE_PROPS, "plans"),
    ]);
    let calls = Rc::new(Cell::new(0));
    let response = Inertia::response(
        "Billing",
        InertiaProps::new().once("plans", {
            let calls = Rc::clone(&calls);
            move || {
                calls.set(calls.get() + 1);
                json!(["basic"])
            }
        }),
    )
    .into_page("/billing", Some("version-1".into()), &request)
    .unwrap();
    let value = serde_json::to_value(response).unwrap();

    assert_eq!(calls.get(), 1);
    assert_eq!(value["props"]["plans"], json!(["basic"]));
}

#[test]
fn lazy_route_prop_roots_block_shared_props_even_when_omitted() {
    let request = request_context_from(&[]);
    let response = Inertia::response(
        "Dashboard",
        InertiaProps::new().optional("auth", || json!({ "user": { "name": "Route" } })),
    )
    .into_page("/dashboard", Some("version-1".into()), &request)
    .unwrap()
    .with_shared_props(vec![
        (
            "auth.user",
            json!({
                "name": "Shared"
            }),
        ),
        ("appName", json!("Demo")),
    ]);
    let value = serde_json::to_value(response).unwrap();

    assert_eq!(value["props"]["auth"]["user"]["name"], "Shared");
    assert_eq!(value["props"]["appName"], "Demo");
    assert_eq!(value["sharedProps"], json!(["auth", "appName"]));
}

#[test]
fn empty_shared_props_are_a_noop() {
    let page =
        Page::new("Empty", Value::Null, "/empty").with_shared_props(Vec::<(&str, Value)>::new());
    let value = serde_json::to_value(page).unwrap();

    assert_eq!(value["props"], Value::Null);
    assert!(value.get("sharedProps").is_none());
}

#[test]
fn page_equality_ignores_internal_route_prop_tracking() {
    let request = request_context_from(&[]);
    let response = Inertia::response(
        "Users",
        json!({
            "auth": {
                "user": {
                    "name": "Ada"
                }
            }
        }),
    )
    .into_page("/users", Some("version-1".into()), &request)
    .unwrap();
    let manual = Page::from_parts(
        "Users",
        json!({
            "errors": {},
            "auth": {
                "user": {
                    "name": "Ada"
                }
            }
        }),
        "/users",
        Some("version-1".into()),
        PageMetadata::new(),
    );

    assert_eq!(response, manual);
}
