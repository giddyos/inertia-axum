use axum::{extract::State, routing::get, Router};
use inertia_axum::prelude::*;

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
    let frontend = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("frontend");
    let inertia = InertiaApp::vite(frontend).build()?;
    let state = AppState {
        app_name: "Inertia Axum",
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3001").await?;

    axum::serve(listener, app(state, inertia)).await?;
    Ok(())
}
