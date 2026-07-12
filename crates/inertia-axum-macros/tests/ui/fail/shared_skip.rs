use inertia_axum_macros::InertiaProps;

#[derive(InertiaProps)]
#[inertia(shared, typegen(skip))]
struct Shared { title: String }

fn main() {}
