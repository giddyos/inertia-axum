//! Automatic page and props root export tests.

#![cfg(feature = "typegen")]
#![allow(dead_code)]

use inertia_axum::{
    __private::typegen::{Config, TS},
    InertiaPage, InertiaProps, InertiaType, Prop,
};
use serde::Serialize;

#[derive(Serialize, InertiaType)]
struct Item {
    id: u32,
}

struct RuntimeOnly;

#[derive(InertiaPage)]
#[inertia(
    component = "Items/Index",
    rename_all = "camelCase",
    typegen(name = "ItemsIndexProps", path = "pages/Items/Index.ts")
)]
struct ItemsPage {
    item: Item,
    deferred_item: Prop<Option<Item>>,
    subtitle: Option<String>,
    #[inertia(rename = "canEdit")]
    can_edit_source: bool,
    #[inertia(skip)]
    runtime_only: RuntimeOnly,
}

#[derive(InertiaProps)]
#[inertia(shared, rename_all = "camelCase")]
struct AppSharedProps {
    app_name: String,
    #[ts(type = "string")]
    build: u32,
}

#[test]
fn root_proxy_preserves_runtime_wire_semantics() {
    let config = Config::default();
    let page = __InertiaPageTypegenItemsPage::decl(&config);
    assert!(page.contains("deferredItem?: Item | null"));
    assert!(page.contains("subtitle: string | null"));
    assert!(page.contains("canEdit: boolean"));
    assert!(!page.contains("runtime_only"));
    let shared = __InertiaPropsTypegenAppSharedProps::decl(&config);
    assert!(shared.contains("appName: string"));
    assert!(shared.contains("build: string"));
}
