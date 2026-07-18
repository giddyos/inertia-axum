use inertia_macros::InertiaPage;

#[derive(InertiaPage)]
#[inertia(component = "Errors")]
struct Reserved { errors: String }

fn main() {}
