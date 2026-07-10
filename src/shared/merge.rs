//! Framework-neutral shared-prop path expansion and collision-safe insertion.
//!
//! Axum provider registration consumes these crate-private primitives. Empty
//! dotted segments are ignored and existing route-owned or non-object roots
//! are never overwritten.

use serde_json::{Map, Value};

pub(crate) fn prop_root(prop: &str) -> &str {
    prop.split_once('.')
        .map(|(root, _suffix)| root)
        .unwrap_or(prop)
}

pub(crate) fn ensure_errors_prop(props: &mut Map<String, Value>) {
    props
        .entry("errors")
        .or_insert_with(|| Value::Object(Map::new()));
}

pub(crate) fn insert_shared_prop_path(
    props: &mut Map<String, Value>,
    key: &str,
    value: Value,
) -> bool {
    let mut segments = key
        .split('.')
        .filter(|segment| !segment.is_empty())
        .peekable();
    if segments.peek().is_none() {
        return false;
    }
    let mut object = props;
    while let Some(segment) = segments.next() {
        if segments.peek().is_none() {
            return match object.entry(segment.to_owned()) {
                serde_json::map::Entry::Vacant(entry) => {
                    entry.insert(value);
                    true
                }
                serde_json::map::Entry::Occupied(_) => false,
            };
        }
        let nested = object
            .entry(segment.to_owned())
            .or_insert_with(|| Value::Object(Map::new()));
        let Value::Object(next) = nested else {
            return false;
        };
        object = next;
    }
    false
}
