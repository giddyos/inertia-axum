use inertia_axum::PropKey;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

/// An owned Inertia page value with typed prop assertions.
#[derive(Clone, Debug)]
pub struct TestPage {
    pub(crate) value: Value,
}

impl TestPage {
    pub(crate) fn new(value: Value) -> Self {
        Self { value }
    }

    /// Returns the complete page JSON value.
    pub fn value(&self) -> &Value {
        &self.value
    }

    /// Decodes a top-level prop through its generated typed key.
    pub fn prop<T: DeserializeOwned>(&self, key: PropKey<T>) -> T {
        serde_json::from_value(self.prop_value(key.name()).clone())
            .unwrap_or_else(|error| panic!("prop `{}` did not decode: {error}", key.name()))
    }

    /// Asserts that a typed prop was omitted from the response.
    pub fn assert_missing<T>(&self, key: PropKey<T>) -> &Self {
        assert!(
            self.props().get(key.name()).is_none(),
            "prop `{}` was present",
            key.name()
        );
        self
    }

    /// Decodes one flash value.
    pub fn flash<T: DeserializeOwned>(&self, key: &str) -> T {
        let value = self
            .value
            .get("flash")
            .and_then(|flash| flash.get(key))
            .unwrap_or_else(|| panic!("flash `{key}` was missing"));
        serde_json::from_value(value.clone())
            .unwrap_or_else(|error| panic!("flash `{key}` did not decode: {error}"))
    }

    /// Asserts that a flash value is absent.
    pub fn assert_no_flash(&self, key: &str) -> &Self {
        assert!(
            self.value
                .get("flash")
                .and_then(|flash| flash.get(key))
                .is_none(),
            "flash `{key}` was present"
        );
        self
    }

    /// Asserts that a dotted validation error exists under the shared `errors` prop.
    pub fn assert_error(&self, path: &str) -> &Self {
        let mut current = self
            .props()
            .get("errors")
            .unwrap_or_else(|| panic!("page had no `errors` prop"));
        for segment in path.split('.') {
            current = current
                .get(segment)
                .unwrap_or_else(|| panic!("validation error `{path}` was missing"));
        }
        assert!(!current.is_null(), "validation error `{path}` was null");
        self
    }

    /// Asserts append-merge metadata for a typed prop.
    pub fn assert_appends<T>(&self, key: PropKey<T>) -> &Self {
        self.assert_merge_metadata_contains("mergeProps", key.name())
    }
    /// Asserts prepend-merge metadata for a typed prop.
    pub fn assert_prepends<T>(&self, key: PropKey<T>) -> &Self {
        self.assert_merge_metadata_contains("prependProps", key.name())
    }
    /// Asserts deep-merge metadata for a typed prop.
    pub fn assert_deep_merges<T>(&self, key: PropKey<T>) -> &Self {
        self.assert_metadata_contains("deepMergeProps", key.name())
    }

    /// Asserts the match key emitted for a merge prop.
    pub fn assert_matches_on<T>(&self, key: PropKey<T>, field: &str) -> &Self {
        self.assert_metadata_contains("matchPropsOn", &format!("{}.{}", key.name(), field))
    }

    /// Asserts that a failed prop was rescued and omitted.
    pub fn assert_rescued<T>(&self, key: PropKey<T>) -> &Self {
        self.assert_metadata_contains("rescuedProps", key.name())
    }

    /// Asserts that a typed prop is advertised in a deferred group.
    pub fn assert_deferred<T>(&self, group: &str, key: PropKey<T>) -> &Self {
        let values = self
            .value
            .get("deferredProps")
            .and_then(|groups| groups.get(group))
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("deferred group `{group}` was missing"));
        assert!(
            values
                .iter()
                .any(|value| value.as_str() == Some(key.name())),
            "deferred group `{group}` did not contain `{}`",
            key.name()
        );
        self
    }

    /// Asserts that a typed prop is advertised under a stable once cache key.
    pub fn assert_once<T>(&self, once_key: &str, key: PropKey<T>) -> &Self {
        let prop = self
            .value
            .get("onceProps")
            .and_then(|props| props.get(once_key))
            .and_then(|value| value.get("prop"))
            .and_then(Value::as_str);
        assert_eq!(
            prop,
            Some(key.name()),
            "once key `{once_key}` did not describe `{}`",
            key.name()
        );
        self
    }

    /// Asserts that reset suppressed every merge and scroll directive for a prop.
    pub fn assert_reset<T>(&self, key: PropKey<T>) -> &Self {
        let name = key.name();
        let nested = format!("{name}.");
        for field in ["mergeProps", "prependProps", "deepMergeProps"] {
            assert!(
                !self
                    .value
                    .get(field)
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .any(|value| value == name || value.starts_with(&nested)),
                "reset prop `{name}` remained in `{field}`"
            );
        }
        assert!(
            self.value
                .get("scrollProps")
                .and_then(|value| value.get(name))
                .is_none(),
            "reset prop `{name}` retained scroll metadata"
        );
        self
    }

    /// Asserts that infinite-scroll metadata exists for a typed prop.
    pub fn assert_scroll<T>(&self, key: PropKey<T>) -> &Self {
        assert!(
            self.value
                .get("scrollProps")
                .and_then(|value| value.get(key.name()))
                .is_some(),
            "scroll metadata for `{}` was missing",
            key.name()
        );
        self
    }

    /// Asserts that the page requests encrypted browser history.
    pub fn assert_encrypts_history(&self) -> &Self {
        assert_eq!(self.value.get("encryptHistory"), Some(&Value::Bool(true)));
        self
    }

    /// Accepts both string and numeric expected versions.
    pub fn assert_version(&self, expected: impl Serialize) -> &Self {
        let expected = serde_json::to_value(expected).expect("version must serialize");
        assert_eq!(
            self.value.get("version"),
            Some(&expected),
            "page version differed"
        );
        self
    }

    fn props(&self) -> &serde_json::Map<String, Value> {
        self.value
            .get("props")
            .and_then(Value::as_object)
            .expect("page `props` was not an object")
    }

    fn prop_value(&self, name: &str) -> &Value {
        self.props()
            .get(name)
            .unwrap_or_else(|| panic!("prop `{name}` was missing"))
    }

    fn assert_metadata_contains(&self, field: &str, expected: &str) -> &Self {
        let values = self
            .value
            .get(field)
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("page metadata `{field}` was missing"));
        assert!(
            values.iter().any(|value| value.as_str() == Some(expected)),
            "page metadata `{field}` did not contain `{expected}`"
        );
        self
    }

    fn assert_merge_metadata_contains(&self, field: &str, expected: &str) -> &Self {
        let values = self
            .value
            .get(field)
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("page metadata `{field}` was missing"));
        let nested = format!("{expected}.");
        assert!(
            values
                .iter()
                .filter_map(Value::as_str)
                .any(|value| value == expected || value.starts_with(&nested)),
            "page metadata `{field}` did not contain `{expected}`"
        );
        self
    }
}
