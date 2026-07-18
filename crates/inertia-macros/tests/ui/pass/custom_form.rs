use inertia_macros::InertiaForm;

fn validate(form: &TokenForm) -> Result<(), ia::Errors> {
    if form.token.is_empty() { Err(ia::Errors::field("token", "required")) } else { Ok(()) }
}

#[derive(InertiaForm)]
#[inertia(validate_with = "validate", old_input, redact = "token")]
struct TokenForm {
    title: String,
    token: String,
}

fn main() {
    let form = TokenForm { title: "Hi".to_owned(), token: String::new() };
    let _ = ia::Validate::validate(&form);
    let _ = ia::Validate::old_input(&form);
}
