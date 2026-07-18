# Inertia for Rust web frameworks

[![CI](https://github.com/giddyos/inertia-axum/actions/workflows/ci.yaml/badge.svg)](https://github.com/giddyos/inertia-axum/actions/workflows/ci.yaml)
[![crates.io](https://img.shields.io/crates/v/inertia-axum.svg)](https://crates.io/crates/inertia-axum)
[![docs.rs](https://docs.rs/inertia-axum/badge.svg)](https://docs.rs/inertia-axum/latest/inertia_axum/)

Framework-neutral Inertia.js v3 runtime and first-party adapters for Axum,
Actix Web, and Rocket. Build server-driven Svelte, React, or Vue applications
without a separate JSON API.

- Dynamic `page!` responses and strongly typed pages and props
- Immediate, shared, lazy, optional, deferred, rescued, merge, scroll, and once data
- Redirect-based validation, old input, error bags, and flash values
- Vite assets with client-side rendering, server-side rendering, or a fully
  embedded production frontend
- In-process page, redirect, deferred-data, cookie, and SSR testing

Choose `inertia-axum` for Axum, `inertia-actix` for Actix Web, or
`inertia-rocket` for Rocket. All three adapters delegate request negotiation,
prop selection, assets, transient data, and SSR to the same `inertia-core`
runtime and pass the same conformance suite.

## Axum

```rust
use axum::{Router, routing::get};
use inertia_axum::prelude::*;
use std::convert::Infallible;

async fn load_stats() -> Result<usize, Infallible> {
    // Simulate a slow database query or calculation.
    tokio::time::sleep(std::time::Duration::from_millis(750)).await;

    Ok(12)
}

async fn home() -> DynamicPage {
    page!("Home", {
        greeting: "Hello",
        // Defer slow work so it does not delay the initial page response.
        stats: defer(load_stats),
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    
    let inertia = InertiaApp::vite("frontend").build()?;

    let app = Router::new().route("/", get(home)).inertia(inertia);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;

    axum::serve(listener, app).await?;
    Ok(())
}
```

## Actix Web

Actix handlers can finalize through the asynchronous extractor API:

```rust
use actix_web::{App, HttpServer, web};
use inertia_actix::{
    Inertia, InertiaApp, InertiaMiddleware, Result as InertiaResult, assets,
};

async fn home(inertia: Inertia) -> InertiaResult {
    inertia.render("Home", serde_json::json!({ "greeting": "Hello" })).await
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let inertia = InertiaApp::vite("frontend")
        .build()
        .map_err(std::io::Error::other)?;

    HttpServer::new(move || {
        App::new()
            .route("/", web::get().to(home))
            .app_data(web::Data::new(inertia.clone()))
            .wrap(InertiaMiddleware::new(inertia.clone()))
            .configure(assets(inertia.clone()))
    })
    .bind(("127.0.0.1", 3000))?
    .run()
    .await
}
```

## Rocket

Rocket installs the app, response fairing, and runtime-mounted assets together:

```rust
use inertia_rocket::{
    Inertia, InertiaApp, InertiaFairing, Result as InertiaResult,
};

#[rocket::get("/")]
async fn home(inertia: Inertia<'_>) -> InertiaResult {
    inertia
        .render("Home", serde_json::json!({ "greeting": "Hello" }))
        .await
}

#[rocket::launch]
fn rocket() -> _ {
    let inertia = InertiaApp::vite("frontend")
        .build()
        .expect("valid Inertia configuration");

    rocket::build()
        .attach(InertiaFairing::new(inertia))
        .mount("/", rocket::routes![home])
}
```

- [Quick start](docs/content/docs/getting-started/quick-start.mdx)
- [Actix Web setup](docs/content/docs/getting-started/actix-web.mdx)
- [Rocket setup](docs/content/docs/getting-started/rocket.mdx)
- [Runnable examples](examples)
- [Rust API documentation](https://docs.rs/inertia-axum/latest/inertia_axum/)
- [Migration from 0.5](docs/content/docs/migrations/from-0-5.mdx)

`1.0.0-alpha.1` is an alpha release with an MSRV of Rust 1.88. Pin the exact version while the API approaches 1.0.
