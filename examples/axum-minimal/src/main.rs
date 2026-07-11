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
    let frontend = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("frontend");
    let app = Router::new()
        .route("/todos", get(index))
        .inertia(InertiaApp::vite(frontend).build()?);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3001").await?;
    axum::serve(listener, app).await?;
    Ok(())
}
