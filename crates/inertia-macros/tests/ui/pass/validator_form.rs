use inertia_macros::InertiaForm;

#[derive(validator::Validate, InertiaForm)]
#[inertia(validator = "validator")]
struct CreateUser {
    #[validate(length(min = 1))]
    name: String,
}

fn main() {
    let form = CreateUser { name: String::new() };
    let _ = ia::Validate::validate(&form);
}
