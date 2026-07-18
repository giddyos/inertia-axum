//! Minimal Rocket application using the framework-neutral Inertia runtime.

use inertia_rocket::{DynamicPage, InertiaApp, InertiaFairing, page};

#[rocket::get("/")]
fn index() -> DynamicPage {
    page!("Home", {
        message: "Rendered by Rocket through Inertia.",
    })
}

#[rocket::launch]
fn rocket() -> _ {
    let inertia = InertiaApp::vite("frontend")
        .dev_server("http://localhost:5173")
        .build()
        .expect("valid Inertia configuration");

    rocket::build()
        .attach(InertiaFairing::new(inertia))
        .mount("/", rocket::routes![index])
}
