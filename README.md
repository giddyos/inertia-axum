# inertia-axum

[![CI](https://github.com/giddyos/inertia-axum/actions/workflows/ci.yaml/badge.svg)](https://github.com/giddyos/inertia-axum/actions/workflows/ci.yaml)
[![crates.io](https://img.shields.io/crates/v/inertia-axum.svg)](https://crates.io/crates/inertia-axum)
[![docs.rs](https://docs.rs/inertia-axum/badge.svg)](https://docs.rs/inertia-axum/latest/inertia_axum/)

The Inertia.js v3 server adapter for Axum: build server-driven Svelte, React, or Vue applications without maintaining a separate JSON API.

## Features

- Dynamic `page!` responses and derived typed pages and props
- Shared, lazy, optional, deferred, rescued, merge, scroll, and once data
- Redirect-based validation, old input, error bags, and flash values
- Built-in, marker-based, Askama, and custom root documents
- Vite assets plus managed or external server-side rendering
- In-process page, redirect, cookie, deferred-data, and SSR testing

## Quick start

Add the server dependencies:

```toml
[dependencies]
axum = { version = "0.8.9", features = ["http1", "tokio"] }
inertia-axum = "1.0.0-alpha.1"
tokio = { version = "1", features = ["macros", "net", "rt-multi-thread"] }
```

Return an Inertia page from a normal Axum handler and install the application on the router:

```rust,no_run
use axum::{routing::get, Router};
use inertia_axum::prelude::*;

async fn index() -> DynamicPage {
    page!("Home", { greeting: "Hello" })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let inertia = InertiaApp::vite("frontend").build()?;
    let app = Router::new()
        .route("/", get(index))
        .inertia(inertia);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    axum::serve(listener, app).await?;
    Ok(())
}
```

The [under-ten-minute guide](docs/content/docs/getting-started/quick-start.mdx) creates the Svelte, React, or Vue frontend and its Vite manifest. A custom root template is not required.

## Documentation

- [Full documentation](docs/content/docs/index.mdx)
- [Getting started](docs/content/docs/getting-started/index.mdx)
- [Server-side rendering](docs/content/docs/ssr/index.mdx)
- [Examples](examples)
- [Crate on crates.io](https://crates.io/crates/inertia-axum)
- [Rust API documentation](https://docs.rs/inertia-axum/latest/inertia_axum/)

## Stability

The minimum supported Rust version is 1.88. Version `1.0.0-alpha.1` is an alpha release; pin it and review the [migration guide](docs/content/docs/migrations/from-0-5.mdx) when upgrading from 0.5.
