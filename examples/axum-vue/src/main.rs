#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let inertia = axum_vue::build_inertia().await?;
    let app = axum_vue::router(axum_vue::seeded_state(), inertia);
    let address = std::env::var("ADDR").unwrap_or_else(|_| "127.0.0.1:3004".to_owned());
    let listener = tokio::net::TcpListener::bind(&address).await?;
    println!("listening on http://{address}/todos");
    axum::serve(listener, app).await?;
    Ok(())
}
