use ia::InertiaType;
use serde::Serialize;

#[derive(Serialize, InertiaType)]
struct Conditional {
    #[serde(skip_serializing_if = "String::is_empty")]
    value: String,
}

fn main() {}
