use inertia_axum_macros::InertiaPage;

#[derive(InertiaPage)]
#[inertia(component = "Home", shared)]
struct Home { title: String }

fn main() {}
