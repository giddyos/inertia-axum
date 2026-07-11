# inertia-axum

[![CI](https://github.com/giddyos/inertia-axum/actions/workflows/ci.yaml/badge.svg)](https://github.com/giddyos/inertia-axum/actions/workflows/ci.yaml)

[Inertia.js](https://inertiajs.com/) adapter support for Axum applications.
Build server-driven applications with the Axum routing, state, extractors, and
middleware you already use—without maintaining a separate JSON API.

`inertia-axum` provides Axum responses, typed and dynamic pages, shared data,
deferred and partial props, redirect-based form validation, in-process testing,
and Inertia v3 protocol support. The minimum supported Rust version is 1.88.
See the [protocol support matrix](docs/protocol-support.md) for current coverage.

## Installation

```toml
[dependencies]
axum = "0.8.9"
inertia-axum = "1.0.0-alpha.1"
serde = { version = "1", features = ["derive"] }
tokio = { version = "1", features = ["macros", "net", "rt-multi-thread", "sync"] }

[dev-dependencies]
inertia-axum-test = "1.0.0-alpha.1"
```

## A stateful Todo application

This is an ordinary stateful Axum application. Inertia supplies typed responses,
form extraction, and a router layer; it does not replace Axum's application
structure. The compiled version lives in
[`examples/todo/src/lib.rs`](examples/todo/src/lib.rs).

```rust
use axum::{extract::State, routing::get, Router};
use inertia_axum::{Errors, prelude::*};
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, sync::Arc};
use tokio::sync::RwLock;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Todo {
    pub id: u64,
    pub title: String,
    pub completed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
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

async fn index(State(state): State<AppState>) -> TodosPage {
    let todos = state.todos.read().await.clone();
    let stats_state = state.clone();

    TodosPage {
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
    }
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
```

## Start the server

Keep runtime configuration outside `app` so the same router is reusable in
tests.

```rust,ignore
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let inertia = InertiaApp::vite("frontend")
        // Keeps validation errors and flash data across redirects.
        // Use an encrypted or session-backed store in production.
        .transient(MemoryTransient::new())
        .build()?;

    let app = app(AppState::seeded(), inertia);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    axum::serve(listener, app).await?;
    Ok(())
}
```

`MemoryTransient` is useful for examples and deterministic tests. Production
applications should use encrypted cookie or session-backed transient storage;
the crate intentionally has no insecure default.

## Deferred props on the client

The first response contains `todos` and advertises `stats` as deferred. The
Inertia client then requests `stats` after the page renders. The server does not
calculate it during the initial response.

```svelte
<script>
    import { Deferred } from '@inertiajs/svelte'

    let { todos = [], stats, errors = {} } = $props()
</script>

<ul>
    {#each todos as todo}
        <li>{todo.title}</li>
    {/each}
</ul>

<Deferred data="stats">
    {#snippet fallback()}
        <p>Loading summary…</p>
    {/snippet}

    <p>{stats.remaining} remaining</p>
</Deferred>
```

## Validation

```svelte
<script>
    import { router } from '@inertiajs/svelte'

    let { errors = {} } = $props()
    let title = $state('')

    function submit(event) {
        event.preventDefault()
        router.post('/todos', { title }, { onSuccess: () => { title = '' } })
    }
</script>

<form onsubmit={submit}>
    <input bind:value={title} aria-label="Todo title">
    {#if errors.title}<p>{errors.title}</p>{/if}
    <button type="submit">Add todo</button>
</form>
```

Validation failures redirect back and populate `page.props.errors`; they are
not returned as a special `422` Inertia JSON response. Use
`error_bag = "createTodo"` when a page contains multiple forms with overlapping
field names.

## Test the same application

`TestApp` makes real in-process requests, preserves redirect cookies, and uses
the prop keys generated by `InertiaPage`.

```rust,ignore
let initial = app.inertia_get("/todos").send().await.assert_page::<TodosPage>();
initial
    .assert_deferred("default", TodosPage::STATS)
    .assert_missing(TodosPage::STATS);

let partial = app
    .inertia_get("/todos")
    .only(TodosPage::STATS)
    .send()
    .await
    .assert_page::<TodosPage>();
let stats: TodoStats = partial.prop(TodosPage::STATS);
assert_eq!(stats.remaining, 1);
partial.assert_missing(TodosPage::TODOS);

let redirect = app
    .inertia_post("/todos")
    .header("referer", "/todos")
    .form(&[("title", "")])
    .send()
    .await
    .assert_see_other("/todos");
redirect.follow().await.assert_page::<TodosPage>().assert_error("title");
```

Run the complete focused tests with
`cargo test -p inertia-axum-example-todo`.

## Examples

The first two examples run in a browser; the remaining examples are compact
in-process applications exercised through tests.

| Example | Purpose | Command |
| --- | --- | --- |
| [`axum-minimal`](examples/axum-minimal) | Smallest Axum router, `AppState`, initial HTML, and Inertia JSON | `cargo run -p axum-minimal` |
| [`axum-svelte`](examples/axum-svelte) | Axum, Vite, Svelte 5, automatic deferred loading, and validation UI | `cargo run -p axum-svelte` |
| [`todo`](examples/todo) | Canonical typed page, validation, deferred prop, and `TestApp` tests | `cargo test -p inertia-axum-example-todo` |
| [`incident-board`](examples/incident-board) | Advanced rescue, merge, scroll, flash, and external-location behavior | `cargo test -p inertia-axum-example-incident-board` |
| [`observatory`](examples/observatory) | Protocol regression fixture for once props, reset, deep merge, and redaction | `cargo test -p inertia-axum-example-observatory` |

## More APIs and reference material

The API documentation covers [typed shared data](https://docs.rs/inertia-axum/latest/inertia_axum/trait.Share.html),
[optional and partial props](https://docs.rs/inertia-axum/latest/inertia_axum/struct.Prop.html),
[merge and infinite-scroll props](https://docs.rs/inertia-axum/latest/inertia_axum/struct.ScrollPage.html),
[once props](https://docs.rs/inertia-axum/latest/inertia_axum/fn.once.html),
[rescued prop errors](https://docs.rs/inertia-axum/latest/inertia_axum/struct.Prop.html),
[custom asset providers](https://docs.rs/inertia-axum/latest/inertia_axum/trait.AssetProvider.html),
and [custom root views](https://docs.rs/inertia-axum/latest/inertia_axum/trait.RootView.html).
Also see the [migration guide](docs/migration-from-0.5.md) and
[protocol support matrix](docs/protocol-support.md).

### Compatibility API

`InertiaRequest`, `SharedProps`, and `VersionLayer` remain available for
migration compatibility. New applications should begin with `InertiaApp`, typed
pages, typed sharing, and the response helpers exported by the prelude.
