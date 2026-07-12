use axum::{extract::State, routing::get, Router};
use inertia_axum::{prelude::*, Errors};
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, path::PathBuf, sync::Arc};
use tokio::sync::RwLock;

#[derive(Clone, Serialize, InertiaType)]
struct Todo {
    id: u64,
    title: String,
}

#[derive(Serialize, InertiaType)]
struct TodoStats {
    total: usize,
    remaining: usize,
}

#[derive(Clone)]
pub struct AppState {
    todos: Arc<RwLock<Vec<Todo>>>,
}

#[derive(InertiaPage)]
#[inertia(component = "Todos/Index")]
struct TodosPage {
    todos: Vec<Todo>,
    stats: Prop<TodoStats>,
}

async fn index(State(state): State<AppState>) -> TodosPage {
    let todos = state.todos.read().await.clone();
    let stats_state = state.clone();
    TodosPage {
        todos,
        stats: defer(move || async move {
            let todos = stats_state.todos.read().await;
            Ok::<_, Infallible>(TodoStats {
                total: todos.len(),
                remaining: todos.len(),
            })
        }),
    }
}

async fn private_todos(State(state): State<AppState>) -> TodosPage {
    index(State(state)).await
}

async fn preview(State(state): State<AppState>) -> TodosPage {
    index(State(state)).await
}

#[derive(Deserialize, InertiaForm)]
#[inertia(validate_with = "validate_todo")]
struct CreateTodo {
    title: String,
}

fn validate_todo(input: &CreateTodo) -> Result<(), Errors> {
    if input.title.trim().is_empty() {
        Err(Errors::field("title", "Enter a todo title"))
    } else {
        Ok(())
    }
}

async fn store(State(state): State<AppState>, Validated(input): Validated<CreateTodo>) -> Redirect {
    let mut todos = state.todos.write().await;
    let id = todos.last().map_or(1, |todo| todo.id + 1);
    todos.push(Todo {
        id,
        title: input.title.trim().to_owned(),
    });
    Redirect::to("/todos")
}

pub fn frontend_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("react-app")
}

pub async fn build_inertia() -> Result<InertiaApp, inertia_axum::StartError> {
    InertiaApp::vite(frontend_root())
        .entry("src/app.jsx")
        .build_dir("../public/build")
        .public_path("/public/build")
        .ssr("dist/ssr/app.js")
        .transient(MemoryTransient::new())
        .start()
        .await
}

pub fn seeded_state() -> AppState {
    AppState {
        todos: Arc::new(RwLock::new(vec![Todo {
            id: 1,
            title: "Try automatic deferred props".to_owned(),
        }])),
    }
}

pub fn router(state: AppState, inertia: InertiaApp) -> Router {
    Router::new()
        .route("/todos", get(index).post(store))
        .route("/todos/private", get(private_todos).without_ssr())
        .route(
            "/todos/preview",
            get(preview).ssr_when(|context| !context.headers().contains_key("x-force-csr")),
        )
        .with_state(state)
        .inertia(inertia)
}
