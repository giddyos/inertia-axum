use inertia_macros::InertiaForm;

#[derive(InertiaForm)]
#[inertia(old_input, redact = "password")]
struct Invalid { value: String }

fn main() {}
