# inertia-axum

[![CI](https://github.com/giddyos/inertia-axum/actions/workflows/ci.yaml/badge.svg)](https://github.com/giddyos/inertia-axum/actions/workflows/ci.yaml)

The [Inertia.js](https://inertiajs.com/) server adapter for [Axum](https://github.com/tokio-rs/axum). Build server-driven web applications with normal Axum routes, state, extractors, and middleware—without maintaining a separate JSON API.

## Feature overview

- Dynamic `page!` responses and strongly typed pages
- Shared, lazy, deferred, optional, merge, scroll, and once props
- Redirect-based validation and transient flash data
- Built-in, startup-compiled marker, and optional Askama root documents
- CSR and SSR from the same routes
- In-process application testing and Inertia v3 protocol support

The minimum supported Rust version is 1.88. See the [protocol support matrix](docs/protocol-support.md) for detailed coverage.

## Quick start

Add `inertia-axum`, `axum`, and Tokio. No root template file is required:

```rust,no_run
use axum::{extract::State, routing::get, Router};
use inertia_axum::prelude::*;
use std::path::PathBuf;

#[derive(Clone)]
struct AppState {
    app_name: &'static str,
}

async fn index(State(state): State<AppState>) -> DynamicPage {
    page!("Home", {
        app_name: state.app_name,
        message: "Rendered by Axum through Inertia.",
    })
}

fn app(state: AppState, inertia: InertiaApp) -> Router {
    Router::new()
        .route("/", get(index))
        .with_state(state)
        .inertia(inertia)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let inertia = InertiaApp::vite(root.join("frontend")).build()?;
    let state = AppState { app_name: "My app" };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    axum::serve(listener, app(state, inertia)).await?;
    Ok(())
}
```

`page!` names the client component and serializes its object as props. `InertiaApp::vite` loads frontend metadata, while `.inertia(inertia)` installs request parsing and response finalization on the router.

A root template is optional. The built-in root includes the required document metadata, generated asset tags, Inertia head output, and CSR or SSR application mount.

## Root documents

| Strategy | Extra dependency | Template preparation | Best for |
| --- | ---: | --- | --- |
| Built-in root | None | None | Standard applications and fastest setup |
| Marker template | None | Validated once at startup | Custom static HTML documents |
| Askama root | Optional `askama` feature | Compiled with the application | Typed document values and template logic |

The root renderer handles the initial full HTML response. Later Inertia visits return JSON and update the page component without recreating the root document.

### Marker templates

Create `templates/app.html`:

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <meta
      name="viewport"
      content="width=device-width, initial-scale=1"
    >
    <!-- inertia:assets -->
    <!-- inertia:head -->
  </head>
  <body>
    <!-- inertia:mount -->
  </body>
</html>
```

Each marker is required exactly once. File templates are read, validated, and compiled once during startup; requests never reread or reparse them. Restart the application after changing the file.

Select it with `.root_template("templates/app.html")`. The source can be deleted after `build()` and the compiled root continues to serve requests.

Embed a template in the binary when that fits deployment better:

```rust,no_run
# use inertia_axum::InertiaApp;
let inertia = InertiaApp::default_root()
    .root_template_source(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/axum-minimal/templates/app.html"
    )))
    .build()?;
# Ok::<(), inertia_axum::ConfigError>(())
```

### Askama templates

Enable the optional integration:

```toml
inertia-axum = {
    version = "1.0.0-alpha.1",
    features = ["askama"]
}
```

Define the complete typed root factory:

```rust
use inertia_axum::{
    AskamaRoot, AskamaRootContext,
    askama::{self, Template},
};
use std::sync::Arc;

#[derive(Template)]
#[template(path = "app.html", askama = askama)]
struct AppTemplate<'a> {
    inertia: AskamaRootContext<'a>,
    app_name: &'a str,
    description: &'a str,
    locale: &'a str,
}

#[derive(Clone)]
struct AppRoot {
    app_name: Arc<str>,
    description: Arc<str>,
    locale: Arc<str>,
}

impl AppRoot {
    fn new(app_name: impl Into<Arc<str>>, description: impl Into<Arc<str>>) -> Self {
        Self {
            app_name: app_name.into(),
            description: description.into(),
            locale: Arc::from("en"),
        }
    }
}

impl AskamaRoot for AppRoot {
    type Template<'a> = AppTemplate<'a>;

    fn template<'a>(&'a self, inertia: AskamaRootContext<'a>) -> Self::Template<'a> {
        AppTemplate {
            inertia,
            app_name: self.app_name.as_ref(),
            description: self.description.as_ref(),
            locale: self.locale.as_ref(),
        }
    }
}
```

Create `templates/app.html`:

```html
<!doctype html>
<html lang="{{ locale }}">
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    {% if !inertia.has_ssr_head %}<title>{{ app_name }}</title>{% endif %}
    <meta name="description" content="{{ description }}">
    {{ inertia.assets|safe }}
    {{ inertia.head|safe }}
  </head>
  <body>{{ inertia.mount|safe }}</body>
</html>
```

Only `inertia.assets`, `inertia.head`, and `inertia.mount` are trusted generated markup and use `safe`. Application and user values must retain Askama's default HTML escaping.

Register the root:

```rust,ignore
let inertia = InertiaApp::vite("frontend")
    .askama_root(AppRoot::new("Acme", "The Acme application"))
    .build()?;
```

See the copyable [`axum-askama` example](examples/axum-askama) for the same setup as a runnable crate.

The advanced [`RootView`](https://docs.rs/inertia-axum/latest/inertia_axum/trait.RootView.html) API remains available for other engines or fully custom rendering. Custom implementations are responsible for their own performance.

## Page responses

Use `page!` for concise pages or `#[derive(InertiaPage)]` for compiler-checked prop contracts:

```rust
use inertia_axum::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct User { id: u64, name: String }

#[derive(InertiaPage)]
#[inertia(component = "Users/Show")]
struct UserPage { user: User }

async fn show() -> UserPage {
    UserPage { user: User { id: 1, name: "Ada".into() } }
}
```

Use `DynamicPage::new("Users/Show").prop("id", 42)` when response options are conditional.

## Props

Ordinary serializable values resolve immediately. `defer`, `lazy`, `optional`, `always`, `merge`, `scroll`, and `once` control when data is evaluated and how the client applies it.

```rust
use inertia_axum::prelude::*;
use std::convert::Infallible;

async fn dashboard() -> DynamicPage {
    page!("Dashboard", {
        title: "Overview",
        stats: defer(|| async { Ok::<_, Infallible>(vec![12, 8, 5]) }),
    })
}
```

See the [`Prop`](https://docs.rs/inertia-axum/latest/inertia_axum/struct.Prop.html) and [`ScrollPage`](https://docs.rs/inertia-axum/latest/inertia_axum/struct.ScrollPage.html) references.

## Forms and validation

Derive `InertiaForm`, name a validator, and extract `Validated<T>`. Invalid input redirects back before the handler runs.

```rust
use inertia_axum::{Errors, prelude::*};
use serde::Deserialize;

#[derive(Deserialize, InertiaForm)]
#[inertia(validate_with = "validate_signup")]
struct Signup { email: String }

fn validate_signup(input: &Signup) -> Result<(), Errors> {
    input.email.contains('@').then_some(())
        .ok_or_else(|| Errors::field("email", "Enter a valid email"))
}

async fn store(Validated(_input): Validated<Signup>) -> Redirect {
    Redirect::to("/welcome").flash("message", "Account created")
}
```

Configure `MemoryTransient` for examples/tests or encrypted-cookie/session storage in production.

## Shared data

Implement [`Share`](https://docs.rs/inertia-axum/latest/inertia_axum/trait.Share.html) for typed data needed by many pages, such as the authenticated user or notifications, then install it with `.share(provider)`.

## Testing

`inertia-axum-test` sends real in-process requests and provides page-aware assertions, redirect/cookie handling, partial prop selection, and deferred-prop assertions. See [`examples/todo`](examples/todo) for a complete test suite.

## SSR

Enable the `ssr` feature and use async startup:

```rust,no_run
# use inertia_axum::prelude::*;
# async fn example() -> Result<(), inertia_axum::StartError> {
let inertia = InertiaApp::vite("frontend")
    .root_template("templates/app.html")
    .ssr("dist/ssr/app.js")
    .start()
    .await?;
# Ok(())
# }
```

Template validation occurs before an SSR runtime starts. See the [SSR guide](docs/ssr.md) for Node modes, policies, health, testing, and operations.

## Examples

- [`axum-askama`](examples/axum-askama): typed application root with a minimal Vue page
- [`axum-minimal`](examples/axum-minimal): smallest runnable router, root template, `page!`, and state
- [`axum-svelte`](examples/axum-svelte): Svelte 5, production SSR, deferred loading, and validation UI
- [`todo`](examples/todo): typed pages, validation, deferred props, and `TestApp`
- [`incident-board`](examples/incident-board): rescue, merge, scroll, flash, and external locations
- [`observatory`](examples/observatory): once props, reset, deep merge, and redaction

## Reference

- [API documentation](https://docs.rs/inertia-axum/latest/inertia_axum/)
- [Root templates](docs/root-templates.md)
- [Protocol support matrix](docs/protocol-support.md)
- [Server-side rendering](docs/ssr.md)
- [Migration guide from 0.5](docs/migration-from-0.5.md)
- [Custom asset providers](https://docs.rs/inertia-axum/latest/inertia_axum/trait.AssetProvider.html)
- [Custom root views](https://docs.rs/inertia-axum/latest/inertia_axum/trait.RootView.html)
