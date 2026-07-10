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

## Examples

- [`examples/axum-minimal`](examples/axum-minimal): minimal HTML and JSON
  Inertia responses.
- [`examples/axum-svelte`](examples/axum-svelte): Axum + Svelte 5 + Vite with
  shared props, deferred props, optional props, partial reloads, and
  manifest-derived asset versioning.

## Usage

`InertiaRequest` extracts the request context, current URI, request method, and
optional asset version. Add `VersionLayer` for asset-version checks and render
through `InertiaRequest::render`.

```rust
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::{Extension, Router};
use inertia_axum::axum::{InertiaError, InertiaRequest, SharedProps, VersionLayer};
use inertia_axum::Inertia;

#[derive(serde::Serialize)]
struct Hello {
    name: String,
}

async fn hello(request: InertiaRequest) -> Result<Response, InertiaError> {
    request.render(
        Inertia::response("Hello", Hello { name: "world".into() }),
        |context| Html(format!(
            r#"<script data-page="app" type="application/json">{}</script>"#,
            context.data_page()
        )).into_response(),
    )
}

let app = Router::new()
    .route("/hello", get(hello))
    .layer(Extension(SharedProps::new().value("appName", "Demo")))
    .layer(VersionLayer::new("asset-version-1"));
```

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
