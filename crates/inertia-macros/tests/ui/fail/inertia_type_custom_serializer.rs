use ia::InertiaType;
use serde::Serialize;

#[derive(Serialize, InertiaType)]
struct Invoice {
    #[serde(serialize_with = "serialize_total")]
    total: u32,
}

fn serialize_total<S>(value: &u32, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.collect_str(value)
}

fn main() {}
