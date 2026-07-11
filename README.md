# inertia-axum

[![CI](https://github.com/giddyos/inertia-axum/actions/workflows/ci.yaml/badge.svg)](https://github.com/giddyos/inertia-axum/actions/workflows/ci.yaml)

[Inertia.js](https://inertiajs.com/) adapter support for Axum applications.
The crate provides Inertia protocol models, request extraction, page rendering,
shared props, asset version handling, and redirect helpers for Axum.

## Status

inertia-axum supports the core Inertia response flow for Axum:

- HTML first-page responses and JSON Inertia responses.
- Asset version checks and stale-visit handling.
- Inertia v3 metadata for partial reloads, merge props, deferred props, once
  props, history flags, and infinite-scroll metadata.
- Synchronous lazy, optional, deferred, and once prop resolvers.
- Shared props with request-aware providers.
- External-location and method-aware redirect helpers.

The minimum supported Rust version is 1.88.

See the [protocol support matrix](docs/protocol-support.md) for representative
tests and current limitations.

## Installation

```toml
[dependencies]
inertia-axum = { git = "https://github.com/giddyos/inertia-axum" }
axum = "0.8.9"
```

## Minimal Todo application

With a conventional Vite project in `frontend`, the complete server setup is:

```rust,no_run
use axum::{routing::get, Router};
use inertia_axum::prelude::*;

async fn index() -> DynamicPage {
    let todos = [
        "Design the public API",
        "Build the response finalizer",
        "Add integration tests",
    ];

    page!("Todos/Index", { todos })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let app = Router::new()
        .route("/todos", get(index))
        .inertia(InertiaApp::vite("frontend").build()?);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    axum::serve(listener, app).await?;
    Ok(())
}
```

The default Vite conventions are `src/main.ts`, `dist/.vite/manifest.json`, and
the `/build` public path. `VITE_DEV_SERVER_URL` switches startup to development
tags without requiring a manifest. Entry, build directory, public path, root
view, and development URL can all be overridden on the builder.

## Examples

- [`examples/axum-minimal`](examples/axum-minimal): the Todo setup above.
- [`examples/axum-svelte`](examples/axum-svelte): Axum + Svelte 5 + Vite using
  the same convention-based application setup.

## Compatibility API

The `InertiaRequest`, `VersionLayer`, and `SharedProps` APIs remain available
through `inertia_axum::compat` during the 1.0 alpha migration.

Use `VersionLayer::dynamic` for a request-time asset version provider. Keep the
provider fast and read a cached value rather than doing blocking I/O there.

## Inertia v3 Helpers

```rust
use inertia_axum::{Inertia, InertiaProps};

let props = InertiaProps::new()
    .value("user", user)
    .lazy("companies", || load_companies())
    .optional("auditTrail", || load_audit_trail())
    .defer("analytics", || load_analytics())
    .once("plans", || load_plans());

Inertia::response("Users/Index", props)
    .always("auth")
    .merge("users");
```

The root crate also exposes `Page`, `PageMetadata`, `RequestContext`,
`InertiaProps`, and `ScopedInertiaProps` for framework-neutral protocol work.

## Shared Props

Register `SharedProps` through an Axum `Extension` layer. Providers receive the
extracted `InertiaRequest` and can read values inserted by other Axum layers.

```rust
use axum::{Extension, Router};
use inertia_axum::axum::SharedProps;

let shared_props = SharedProps::new()
    .value("appName", "My App")
    .prop("auth.user", |request| {
        request.extension::<User>().map(|user| user.summary())
    });

let app = Router::new().layer(Extension(shared_props));
```

Shared props are merged into HTML and JSON page responses. Route props win on
key collisions, and dotted keys such as `auth.user` become nested objects.

## Redirect Helpers

Use `InertiaRequest::location` for external redirects from Inertia visits:

```rust
async fn billing(request: InertiaRequest) -> Result<Response, InertiaError> {
    request.location(Inertia::location("https://billing.example.com"))
}
```

Use `InertiaRequest::redirect` for method-aware application redirects:

```rust
async fn create_user(request: InertiaRequest) -> Result<Response, InertiaError> {
    request.redirect(Inertia::redirect("/users"))
}
```

The Axum integration returns `302 Found` for read-style requests and `303 See
Other` for `POST`, `PUT`, `PATCH`, and `DELETE`.

## Request Helpers

```rust
use inertia_axum::axum::InertiaRequest;

async fn debug(request: InertiaRequest) -> String {
    format!(
        "is_inertia={}, version={:?}",
        request.is_inertia(),
        request.asset_version()
    )
}
```

Raw protocol header constants are available from the crate root, for example
`inertia_axum::X_INERTIA` and `inertia_axum::X_INERTIA_VERSION`.
