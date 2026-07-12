use inertia_axum_macros::InertiaProps;

#[derive(InertiaProps)]
#[inertia(typegen(path = "../outside.ts"))]
struct Props { title: String }

fn main() {}
