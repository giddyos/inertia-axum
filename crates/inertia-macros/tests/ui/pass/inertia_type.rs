use ia::InertiaType;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize, InertiaType)]
#[serde(rename_all = "camelCase")]
struct Profile {
    display_name: String,
}

#[derive(Serialize, InertiaType)]
struct Generic<T>
where
    T: Clone,
{
    value: T,
}

#[derive(Serialize, InertiaType)]
struct Tuple(String, u32);

#[derive(Serialize, InertiaType)]
struct Unit;

#[derive(Serialize, InertiaType)]
#[serde(tag = "kind", content = "data")]
enum Status {
    Ready,
    Loaded { profile: Profile },
}

#[derive(Serialize, InertiaType)]
#[serde(untagged)]
enum Value {
    Text(String),
    Count(u32),
}

#[derive(Serialize, InertiaType)]
struct Recursive {
    child: Option<Box<Recursive>>,
}

#[derive(Serialize, InertiaType)]
struct Collections {
    profiles: Vec<Profile>,
    by_name: BTreeMap<String, Profile>,
    maybe: Option<String>,
    #[serde(skip)]
    skipped: String,
    #[ts(rename = "wireName")]
    renamed: bool,
    #[serde(serialize_with = "serialize_code")]
    #[ts(as = "String")]
    code: u32,
}

fn serialize_code<S>(value: &u32, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.collect_str(value)
}

fn main() {}
