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
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let inertia = InertiaApp::vite(root.join("frontend"))
        .root_template(root.join("templates/app.html"))
        .build()?;
    let state = AppState {
        app_name: "Inertia Axum",
    };
    let address = std::env::var("ADDR").unwrap_or_else(|_| "127.0.0.1:3001".to_owned());
    let listener = tokio::net::TcpListener::bind(address).await?;

    axum::serve(listener, app(state, inertia)).await?;
    Ok(())
}
