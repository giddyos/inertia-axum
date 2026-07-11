use axum::{routing::get, Router};
use inertia_axum::prelude::*;
use serde::{Deserialize, Serialize};
use std::{
    convert::Infallible,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TodoDto {
    pub id: u64,
    pub title: String,
    pub completed: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct TodoFilters {
    pub search: Option<String>,
    pub completed: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TodoStats {
    pub total: u64,
    pub completed: u64,
    pub remaining: u64,
}

#[derive(InertiaPage)]
#[inertia(component = "Todos/Index", rename_all = "camelCase")]
pub struct TodosIndexPage {
    pub todos: Vec<TodoDto>,
    pub filters: TodoFilters,
    pub stats: Prop<TodoStats>,
    pub archived: Prop<Vec<TodoDto>>,
    pub can_create: bool,
}

#[derive(Clone)]
pub struct Repository {
    todos: Arc<Vec<TodoDto>>,
    pub searches: Arc<AtomicUsize>,
    pub archived: Arc<AtomicUsize>,
}

impl Repository {
    pub fn fixture() -> Self {
        Self {
            todos: Arc::new(vec![TodoDto {
                id: 1,
                title: "Ship adapter".into(),
                completed: false,
            }]),
            searches: Arc::new(AtomicUsize::new(0)),
            archived: Arc::new(AtomicUsize::new(0)),
        }
    }
}

pub fn app(repository: Repository) -> Router {
    Router::new()
        .route(
            "/todos",
            get(move || {
                let repository = repository.clone();
                async move {
                    repository.searches.fetch_add(1, Ordering::SeqCst);
                    let stats_repo = repository.clone();
                    let archived_repo = repository.clone();
                    TodosIndexPage {
                        todos: repository.todos.as_ref().clone(),
                        filters: TodoFilters::default(),
                        can_create: true,
                        stats: defer(move || async move {
                            let total = stats_repo.todos.len() as u64;
                            Ok::<_, Infallible>(TodoStats {
                                total,
                                completed: 0,
                                remaining: total,
                            })
                        })
                        .group("summary"),
                        archived: optional(move || async move {
                            archived_repo.archived.fetch_add(1, Ordering::SeqCst);
                            Ok::<_, Infallible>(Vec::new())
                        }),
                    }
                }
            }),
        )
        .inertia(InertiaApp::default_root().build().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use inertia_axum_test::TestApp;

    #[tokio::test]
    async fn initial_html_and_selective_loading_are_typed() {
        let repository = Repository::fixture();
        let archived = repository.archived.clone();
        let app = TestApp::new(app(repository));
        app.get("/todos")
            .send()
            .await
            .assert_ok()
            .assert_html()
            .assert_html_page::<TodosIndexPage>();
        assert_eq!(archived.load(Ordering::SeqCst), 0);
        let page = app
            .inertia_get("/todos")
            .only(TodosIndexPage::STATS)
            .send()
            .await
            .assert_page::<TodosIndexPage>();
        assert_eq!(page.prop(TodosIndexPage::STATS).total, 1);
        page.assert_missing(TodosIndexPage::TODOS)
            .assert_missing(TodosIndexPage::ARCHIVED);
        app.inertia_get("/todos")
            .only(TodosIndexPage::ARCHIVED)
            .send()
            .await
            .assert_page::<TodosIndexPage>();
        assert_eq!(archived.load(Ordering::SeqCst), 1);
    }
}
