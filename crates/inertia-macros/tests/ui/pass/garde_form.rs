use inertia_macros::InertiaForm;

#[derive(garde::Validate, InertiaForm)]
#[inertia(validator = "garde", error_bag = "createTodo")]
struct CreateTodo {
    #[garde(length(min = 1))]
    title: String,
}

fn main() {
    let form = CreateTodo { title: String::new() };
    let _ = ia::Validate::validate(&form);
}
