use axum::{routing::get, Router};
use inertia_axum::prelude::*;
use std::{
    convert::Infallible,
    time::{SystemTime, UNIX_EPOCH},
};

fn generated_at() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time is after the Unix epoch")
        .as_secs()
}

async fn hello() -> DynamicPage {
    page!("Hello", {
        name: "world",
        message: "Rendered by Axum and hydrated by Svelte through Inertia.",
        appName: "Axum Svelte",
        auth: serde_json::json!({"user":{"name":"Grace Hopper","role":"Example user"}}),
        stats: defer(|| async { Ok::<_, Infallible>(serde_json::json!({"adapter":"Axum","deferred":true,"generatedAt":generated_at()})) }),
        debug: optional(|| async { Ok::<_, Infallible>(serde_json::json!({"partialReload":true,"loadedBy":"X-Inertia-Partial-Data: debug"})) }),
    })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let frontend = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("svelte-app");
    let inertia = InertiaApp::vite(frontend)
        .entry("src/main.js")
        .build_dir("../public/build")
        .public_path("/public/build")
        .build()?;
    let app = Router::new().route("/hello", get(hello)).inertia(inertia);
    let addr = std::env::var("ADDR").unwrap_or_else(|_| "127.0.0.1:3002".to_owned());
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("listening on http://{addr}/hello");
    axum::serve(listener, app).await?;
    Ok(())
}
