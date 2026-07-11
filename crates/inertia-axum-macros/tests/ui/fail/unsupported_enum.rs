use inertia_axum_macros::InertiaPage;

#[derive(InertiaPage)]
#[inertia(component = "Bad")]
enum Unsupported { One }

fn main() {}
