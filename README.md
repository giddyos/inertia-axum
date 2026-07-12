# inertia-axum

[![CI](https://github.com/giddyos/inertia-axum/actions/workflows/ci.yaml/badge.svg)](https://github.com/giddyos/inertia-axum/actions/workflows/ci.yaml)
[![crates.io](https://img.shields.io/crates/v/inertia-axum.svg)](https://crates.io/crates/inertia-axum)
[![docs.rs](https://docs.rs/inertia-axum/badge.svg)](https://docs.rs/inertia-axum/latest/inertia_axum/)

The Inertia.js v3 server adapter for building server-driven Svelte, React, or Vue applications with Axum.

- Dynamic `page!` responses and strongly typed pages and props
- Immediate, shared, lazy, optional, deferred, rescued, merge, scroll, and once data
- Redirect-based validation, old input, error bags, and flash values
- Vite assets with client-side or server-side rendering
- In-process page, redirect, deferred-data, cookie, and SSR testing

```rust
use inertia_axum::prelude::*;
use std::convert::Infallible;

async fn load_stats() -> Result<usize, Infallible> {
    Ok(12)
}

async fn home() -> DynamicPage {
    page!("Home", {
        greeting: "Hello",
        stats: defer(load_stats),
    })
}
```

- [Quick start](docs/content/docs/getting-started/quick-start.mdx)
- [Runnable examples](examples)
- [Rust API documentation](https://docs.rs/inertia-axum/latest/inertia_axum/)
- [Migration from 0.5](docs/content/docs/migrations/from-0-5.mdx)

`1.0.0-alpha.1` is an alpha release with an MSRV of Rust 1.88. Pin the exact version while the API approaches 1.0.
