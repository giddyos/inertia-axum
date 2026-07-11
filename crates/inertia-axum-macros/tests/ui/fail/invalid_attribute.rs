use inertia_axum_macros::InertiaPage;

#[derive(InertiaPage)]
#[inertia(component = "Bad", controller)]
struct Invalid { value: u32 }

fn main() {}
