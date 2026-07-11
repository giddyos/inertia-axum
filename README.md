# inertia-axum

[![CI](https://github.com/giddyos/inertia-axum/actions/workflows/ci.yaml/badge.svg)](https://github.com/giddyos/inertia-axum/actions/workflows/ci.yaml)

The [Inertia.js](https://inertiajs.com/) server adapter for
[Axum](https://github.com/tokio-rs/axum). Build server-driven web applications
with Axum routes, state, extractors, and middleware without maintaining a
separate JSON API.

`inertia-axum` includes:

- dynamic and strongly typed page responses
- shared, lazy, deferred, optional, merge, scroll, and once props
- redirect-based form validation and flash data
- initial HTML and Inertia JSON responses from the same routes
- in-process test helpers through `inertia-axum-test`
- Inertia v3 protocol support

The minimum supported Rust version is 1.88. See the
[protocol support matrix](docs/protocol-support.md) for detailed coverage.

## Quick start

### 1. Install

```toml
[dependencies]
inertia-axum = "1.0.0-alpha.1"
axum = "0.8.9"
tokio = { version = "1", features = ["macros", "net", "rt-multi-thread"] }
```

### 2. Return a page

Use `page!` for a small, untyped response. The first argument is the client
component; the object contains its props.

```rust
use axum::{routing::get, Router};
use inertia_axum::prelude::*;

async fn home() -> DynamicPage {
    let name = "Gideon";
    page!("Home", { name, greeting: "Welcome!" })
}

fn app() -> Router {
    let inertia = InertiaApp::vite("frontend").build()?;

    Router::new()
        .route("/", get(home))
        .inertia(inertia)
}
```

`page!` accepts shorthand props (`name`) and named expressions
(`greeting: "Welcome!"`). It returns `DynamicPage`, which implements Axum's
`IntoResponse`.

### 3. Start Axum

`InertiaApp::vite` reads the frontend build or development-server metadata and
the `.inertia(...)` router extension installs the Inertia layer.

```rust,ignore
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let inertia = InertiaApp::vite("frontend").build()?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;

    axum::serve(listener, app(inertia)).await?;
    Ok(())
}
```

For a runnable project with the frontend mount and Vite configuration, see
[`examples/axum-minimal`](examples/axum-minimal).

## Page responses

Choose the response style that fits the route:

| Style | Best for | Result |
| --- | --- | --- |
| `page!` | small pages and prototypes | `DynamicPage` |
| `#[derive(InertiaPage)]` | application pages with a stable prop contract | a typed response struct |

### Dynamic pages

The macro is the shortest form:

```rust
async fn show() -> DynamicPage {
    page!("Users/Show", { id: 42, active: true })
}
```

The builder is useful when response options are conditional:

```rust
async fn show() -> DynamicPage {
    DynamicPage::new("Users/Show")
        .prop("id", 42)
        .encrypt_history()
}
```

### Typed pages

Derive `InertiaPage` when you want the compiler to check the component's prop
shape and generate reusable prop keys for tests.

```rust
use inertia_axum::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct User {
    id: u64,
    name: String,
}

#[derive(InertiaPage)]
#[inertia(component = "Users/Show")]
struct UserPage {
    user: User,
}

async fn show() -> UserPage {
    UserPage {
        user: User { id: 1, name: "Ada".into() },
    }
}
```

Use `rename_all = "camelCase"` on the `#[inertia(...)]` attribute when Rust
field names should become camel-cased client props.

## Props

Ordinary serializable values are resolved immediately. Wrap expensive or
request-specific values to control when and how they are included.

| Helper | Behavior |
| --- | --- |
| `defer` | loads after the initial page render |
| `lazy` | evaluates only when requested |
| `optional` | appears only in an explicit partial reload |
| `always` | remains present during partial reloads |
| `merge` | merges new data into the existing client prop |
| `scroll` | adds pagination metadata for infinite scrolling |
| `once` | reuses a client-cached prop according to its policy |

Prop helpers work in both typed pages and `page!`:

```rust
use std::convert::Infallible;

async fn dashboard() -> DynamicPage {
    page!("Dashboard", {
        title: "Overview",
        stats: defer(|| async {
            Ok::<_, Infallible>(vec![12, 8, 5])
        }),
    })
}
```

For a typed page, declare the wrapped field as `Prop<T>`:

```rust
#[derive(InertiaPage)]
#[inertia(component = "Dashboard")]
struct DashboardPage {
    title: String,
    stats: Prop<Vec<u64>>,
}
```

See the API documentation for [prop policies](https://docs.rs/inertia-axum/latest/inertia_axum/struct.Prop.html)
and [infinite-scroll props](https://docs.rs/inertia-axum/latest/inertia_axum/struct.ScrollPage.html).

## Forms and validation

Derive `InertiaForm`, name a validator, and extract the form with `Validated`.
Invalid input redirects back with errors before the handler body runs.

```rust
use inertia_axum::{Errors, prelude::*};
use serde::Deserialize;

#[derive(Deserialize, InertiaForm)]
#[inertia(validate_with = "validate_signup")]
struct Signup {
    email: String,
}

fn validate_signup(input: &Signup) -> Result<(), Errors> {
    input.email.contains('@')
        .then_some(())
        .ok_or_else(|| Errors::field("email", "Enter a valid email"))
}

async fn store(Validated(input): Validated<Signup>) -> Redirect {
    // Persist `input` here.
    Redirect::to("/welcome").flash("message", "Account created")
}
```

Configure transient storage to carry errors, old input, and flash data across
redirects:

```rust
let inertia = InertiaApp::vite("frontend")
    .transient(MemoryTransient::new())
    .build()?;
```

`MemoryTransient` is suitable for examples and deterministic tests. Production
applications should use encrypted-cookie or session-backed transient storage;
the crate intentionally has no insecure default.

For pages with multiple forms sharing field names, set an error bag with
`#[inertia(error_bag = "signup")]`.

## Shared data

Implement `Share` for data that should be available to multiple pages, such as
the authenticated user or a global notification. Shared props are combined
with route props by the Inertia layer.

See [typed shared data](https://docs.rs/inertia-axum/latest/inertia_axum/trait.Share.html)
for the `Share` and `ShareContext` APIs.

## Testing

Add the test helper crate:

```toml
[dev-dependencies]
inertia-axum-test = "1.0.0-alpha.1"
```

`TestApp` sends real in-process requests and provides page-aware assertions:

```rust,ignore
let page = TestApp::new(router)
    .inertia_get("/users/1")
    .send()
    .await
    .assert_page::<UserPage>();

let user: User = page.prop(UserPage::USER);
assert_eq!(user.name, "Ada");
```

It can also preserve redirect cookies, follow responses, select partial props,
and assert deferred or missing props. See
[`examples/todo`](examples/todo) for a complete test suite.

## Examples

### Browser applications

| Example | Demonstrates | Run |
| --- | --- | --- |
| [`axum-minimal`](examples/axum-minimal) | smallest router, `page!`, initial HTML, and Inertia JSON | `cargo run -p axum-minimal` |
| [`axum-svelte`](examples/axum-svelte) | Vite, Svelte 5, deferred loading, and validation UI | `cargo run -p axum-svelte` |

### Tested application patterns

| Example | Demonstrates | Run |
| --- | --- | --- |
| [`todo`](examples/todo) | typed pages, validation, deferred props, and `TestApp` | `cargo test -p inertia-axum-example-todo` |
| [`incident-board`](examples/incident-board) | rescue, merge, scroll, flash, and external locations | `cargo test -p inertia-axum-example-incident-board` |
| [`observatory`](examples/observatory) | once props, reset, deep merge, and redaction | `cargo test -p inertia-axum-example-observatory` |

## Reference

- [API documentation](https://docs.rs/inertia-axum/latest/inertia_axum/)
- [Protocol support matrix](docs/protocol-support.md)
- [Migration guide from 0.5](docs/migration-from-0.5.md)
- [Custom asset providers](https://docs.rs/inertia-axum/latest/inertia_axum/trait.AssetProvider.html)
- [Custom root views](https://docs.rs/inertia-axum/latest/inertia_axum/trait.RootView.html)
