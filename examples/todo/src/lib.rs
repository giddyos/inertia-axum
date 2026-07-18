use axum::{extract::State, routing::get, Router};
use inertia_axum::{prelude::*, Errors};
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, sync::Arc};
use tokio::sync::RwLock;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, InertiaType)]
pub struct Todo {
    pub id: u64,
    pub title: String,
    pub completed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, InertiaType)]
pub struct TodoStats {
    pub total: usize,
    pub completed: usize,
    pub remaining: usize,
}

#[derive(Clone)]
pub struct AppState {
    todos: Arc<RwLock<Vec<Todo>>>,
}

impl AppState {
    pub fn seeded() -> Self {
        Self {
            todos: Arc::new(RwLock::new(vec![Todo {
                id: 1,
                title: "Rewrite the examples".to_owned(),
                completed: false,
            }])),
        }
    }
}

#[derive(InertiaPage)]
#[inertia(component = "Todos/Index", rename_all = "camelCase")]
pub struct TodosPage {
    pub todos: Vec<Todo>,
    pub stats: Prop<TodoStats>,
}

async fn index(State(state): State<AppState>) -> PendingPage {
    let todos = state.todos.read().await.clone();
    let stats_state = state.clone();

    PendingPage::typed(TodosPage {
        todos,

        // Inertia loads this prop after the initial page has rendered.
        stats: defer(move || async move {
            let todos = stats_state.todos.read().await;
            let completed = todos.iter().filter(|todo| todo.completed).count();

            Ok::<_, Infallible>(TodoStats {
                total: todos.len(),
                completed,
                remaining: todos.len() - completed,
            })
        }),
    })
}

#[derive(Deserialize, InertiaForm)]
#[inertia(validate_with = "validate_todo")]
struct CreateTodo {
    title: String,
}

fn validate_todo(input: &CreateTodo) -> Result<(), Errors> {
    let title = input.title.trim();

    if title.is_empty() {
        return Err(Errors::field("title", "Enter a todo title"));
    }

    if title.len() > 120 {
        return Err(Errors::field(
            "title",
            "Todo titles must be 120 characters or fewer",
        ));
    }

    Ok(())
}

async fn store(
    State(state): State<AppState>,
    // Invalid input redirects back before this handler body runs.
    Validated(input): Validated<CreateTodo>,
) -> Redirect {
    let mut todos = state.todos.write().await;
    let id = todos.last().map_or(1, |todo| todo.id + 1);

    todos.push(Todo {
        id,
        title: input.title.trim().to_owned(),
        completed: false,
    });

    Redirect::to("/todos").flash("message", "Todo created")
}

pub fn app(state: AppState, inertia: InertiaApp) -> Router {
    Router::new()
        .route("/todos", get(index).post(store))
        .with_state(state)
        .inertia(inertia)
}

#[cfg(test)]
mod tests {
    use super::*;
    use inertia_test::TestApp;

    fn test_app() -> TestApp {
        let inertia = InertiaApp::default_root()
            .transient(MemoryTransient::new())
            .build()
            .expect("test Inertia configuration should be valid");

        TestApp::new(app(AppState::seeded(), inertia))
    }

    #[tokio::test]
    async fn stats_are_deferred_until_the_follow_up_request() {
        let app = test_app();
        let initial = app
            .inertia_get("/todos")
            .send()
            .await
            .assert_page::<TodosPage>();

        initial
            .assert_deferred("default", TodosPage::STATS)
            .assert_missing(TodosPage::STATS);

        // TestApp makes the partial request that the browser client makes automatically.
        let partial = app
            .inertia_get("/todos")
            .only(TodosPage::STATS)
            .send()
            .await
            .assert_page::<TodosPage>();
        let stats: TodoStats = partial.prop(TodosPage::STATS);

        assert_eq!(stats.total, 1);
        assert_eq!(stats.remaining, 1);
        partial.assert_missing(TodosPage::TODOS);
    }

    #[tokio::test]
    async fn invalid_input_redirects_back_with_errors() {
        let app = test_app();
        let redirect = app
            .inertia_post("/todos")
            .header("referer", "/todos")
            .form(&[("title", "")])
            .send()
            .await
            .assert_see_other("/todos");

        redirect
            .follow()
            .await
            .assert_page::<TodosPage>()
            .assert_error("title");
    }
}
