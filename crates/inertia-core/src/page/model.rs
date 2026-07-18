//! Serializable Inertia page models and shared-prop page merging.
//!
//! Field declaration order and `Arc<str>` asset versions are part of the wire
//! format and are intentionally preserved here.

use super::metadata::{OnceProp, PageMetadata, ScrollProps};
use crate::{
    AssetVersion,
    shared::{ensure_errors_prop, insert_shared_prop_path, prop_root},
};
use serde::Serialize;
use serde_json::{Map, Value};
use std::borrow::Cow;
use std::collections::BTreeMap;

fn is_false(value: &bool) -> bool {
    !value
}
fn empty_map<K, V>(map: &BTreeMap<K, V>) -> bool {
    map.is_empty()
}

/// A serializable Inertia page object.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub(crate) component: String,
    pub(crate) props: T,
    #[serde(skip_serializing_if = "Map::is_empty")]
    pub(crate) flash: Map<String, Value>,
    pub(crate) url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) version: Option<AssetVersion>,
    #[serde(skip_serializing_if = "is_false")]
    pub(crate) encrypt_history: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub(crate) clear_history: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub(crate) preserve_fragment: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) merge_props: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) prepend_props: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) deep_merge_props: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) match_props_on: Vec<String>,
    #[serde(skip_serializing_if = "empty_map")]
    pub(crate) scroll_props: BTreeMap<String, ScrollProps>,
    #[serde(skip_serializing_if = "empty_map")]
    pub(crate) deferred_props: BTreeMap<String, Vec<String>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) rescued_props: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) shared_props: Vec<String>,
    #[serde(skip_serializing_if = "empty_map")]
    pub(crate) once_props: BTreeMap<String, OnceProp>,
}

impl<T> Page<T> {
    /// Creates a minimal page object.
    pub fn new<C: Into<String>, U: Into<String>>(component: C, props: T, url: U) -> Self {
        Self::from_parts(component, props, url, None, PageMetadata::new())
    }

    /// Creates a page object from explicit parts.
    pub fn from_parts<C: Into<String>, U: Into<String>>(
        component: C,
        props: T,
        url: U,
        version: Option<String>,
        metadata: PageMetadata,
    ) -> Self {
        Self::from_parts_version(
            component,
            props,
            url,
            version.map(AssetVersion::from),
            metadata,
        )
    }

    pub(crate) fn from_parts_version<C: Into<String>, U: Into<String>>(
        component: C,
        props: T,
        url: U,
        version: Option<AssetVersion>,
        metadata: PageMetadata,
    ) -> Self {
        let parts = metadata.into_parts();
        Self {
            component: component.into(),
            props,
            flash: Map::new(),
            url: url.into(),
            version,
            encrypt_history: parts.encrypt_history,
            clear_history: parts.clear_history,
            preserve_fragment: parts.preserve_fragment,
            merge_props: parts.merge_props,
            prepend_props: parts.prepend_props,
            deep_merge_props: parts.deep_merge_props,
            match_props_on: parts.match_props_on,
            scroll_props: parts.scroll_props,
            deferred_props: parts.deferred_props,
            rescued_props: parts.rescued_props,
            shared_props: parts.shared_props,
            once_props: parts.once_props,
        }
    }

    /// Sets the page object's asset version.
    pub fn version<V: Into<AssetVersion>>(mut self, version: V) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Returns the component name.
    pub fn component(&self) -> &str {
        &self.component
    }

    /// Returns the page props.
    pub fn props(&self) -> &T {
        &self.props
    }

    pub(crate) fn set_flash(&mut self, flash: Map<String, Value>) {
        self.flash = flash;
    }

    /// Returns the page URL.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Returns the asset version, if present.
    pub fn asset_version(&self) -> Option<Cow<'_, str>> {
        self.version.as_ref().map(AssetVersion::header_value)
    }
}

impl Page<Value> {
    /// Merges shared props into the page object.
    ///
    /// Existing page props take precedence when keys collide. Dotted keys are
    /// expanded into nested objects, and inserted top-level keys are added to
    /// the page object's `sharedProps` metadata.
    pub fn with_shared_props<I, K>(mut self, shared_props: I) -> Self
    where
        I: IntoIterator<Item = (K, Value)>,
        K: Into<String>,
    {
        let mut shared_props = shared_props.into_iter().peekable();

        if shared_props.peek().is_none() {
            return self;
        }

        if !self.props.is_object() {
            self.props = Value::Object(Map::new());
        }

        let props = self
            .props
            .as_object_mut()
            .expect("props was normalized to an object");
        ensure_errors_prop(props);
        let route_roots = props.keys().cloned().collect::<Vec<_>>();

        for (key, value) in shared_props {
            let key = key.into();
            let root = prop_root(&key);
            if root.is_empty() {
                continue;
            }

            if route_roots.iter().any(|candidate| candidate == root) {
                continue;
            }

            if insert_shared_prop_path(props, &key, value)
                && !self.shared_props.iter().any(|x| x == root)
            {
                self.shared_props.push(root.to_owned());
            }
        }

        self
    }
}
