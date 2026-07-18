//! Rocket application with Vite development and embedded release assets.

use inertia_rocket::{Inertia, InertiaApp, InertiaFairing, Result as InertiaResult};
use serde::Serialize;

#[cfg(not(debug_assertions))]
use inertia_embed::{EmbeddedFrontend, embed_frontend};

#[cfg(not(debug_assertions))]
static FRONTEND: EmbeddedFrontend = embed_frontend! {
    root: "frontend/dist",
    entry: "src/main.ts",
    public_path: "/build",
};

#[derive(Serialize)]
struct HomeProps {
    message: &'static str,
}

#[rocket::get("/")]
async fn index(inertia: Inertia<'_>) -> InertiaResult {
    inertia
        .render(
            "Home",
            HomeProps {
                message: "Hello from one self-contained Rocket binary",
            },
        )
        .await
}

fn inertia() -> Result<InertiaApp, inertia_rocket::ConfigError> {
    #[cfg(debug_assertions)]
    {
        InertiaApp::vite("frontend")
            .entry("src/main.ts")
            .dev_server("http://localhost:5173")
            .build()
    }

    #[cfg(not(debug_assertions))]
    {
        InertiaApp::embedded(&FRONTEND).build()
    }
}

#[rocket::launch]
fn rocket() -> _ {
    let inertia = inertia().expect("valid Inertia configuration");

    rocket::build()
        .attach(InertiaFairing::new(inertia))
        .mount("/", rocket::routes![index])
}
