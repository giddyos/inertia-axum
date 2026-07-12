use inertia_axum::prelude::*;
use serde::Serialize;

#[derive(Serialize, InertiaType)]
pub struct Todo { pub id: u64, pub title: String }

#[derive(InertiaPage)]
#[inertia(component = "Todos/Index", rename_all = "camelCase")]
pub struct TodosPage {
    pub todos: Vec<Todo>,
    pub selected: Prop<Option<Todo>>,
    pub subtitle: Option<String>,
}

#[derive(InertiaProps)]
#[inertia(shared, rename_all = "camelCase")]
pub struct SharedProps { pub app_name: String }
