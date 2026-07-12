use axum::{Router, routing::get};
use inertia_axum::prelude::*;
use serde::Serialize;
use std::convert::Infallible;

#[derive(Serialize)]
struct HomeStats {
    projects: usize,
    tasks: usize,
}

async fn load_stats() -> Result<HomeStats, Infallible> {
    // Simulate a slow database query or calculation.
    tokio::time::sleep(std::time::Duration::from_millis(750)).await;

    Ok(HomeStats {
        projects: 3,
        tasks: 12,
    })
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
