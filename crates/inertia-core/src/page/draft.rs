//! Temporary page assembly before global shared props are merged.
//!
//! Drafts retain boxed route-root tracking so route-owned values block shared
//! insertion without adding visibility-driven clones.

use super::model::Page;
use crate::shared::{ensure_errors_prop, insert_shared_prop_path, prop_root};
use serde_json::{Map, Value};

/// A page under construction before shared props are merged.
#[doc(hidden)]
pub struct PageDraft {
    page: Page<Value>,
    route_roots: Vec<Box<str>>,
    local_shared_roots: Vec<Box<str>>,
}

impl PageDraft {
    pub(crate) fn new(page: Page<Value>, route_roots: Vec<String>) -> Self {
        Self {
            page,
            route_roots: route_roots
                .into_iter()
                .map(String::into_boxed_str)
                .collect(),
            local_shared_roots: Vec::new(),
        }
    }

    pub(crate) fn owns_prop_root(&self, key: &str) -> bool {
        let root = prop_root(key);
        self.route_roots
            .iter()
            .any(|candidate| candidate.as_ref() == root)
    }

    #[doc(hidden)]
    pub fn global_is_blocked(&self, key: &str) -> bool {
        let root = prop_root(key);
        self.route_roots
            .iter()
            .chain(&self.local_shared_roots)
            .any(|candidate| candidate.as_ref() == root)
    }

    pub(crate) fn insert_shared(&mut self, key: &str, value: Value) -> bool {
        if self.owns_prop_root(key) {
            return false;
        }

        if !self.page.props.is_object() {
            self.page.props = Value::Object(Map::new());
        }

        let props = self
            .page
            .props
            .as_object_mut()
            .expect("props was normalized to an object");
        ensure_errors_prop(props);
        if insert_shared_prop_path(props, key, value) {
            let root = prop_root(key);
            if !self
                .page
                .shared_props
                .iter()
                .any(|existing| existing == root)
            {
                self.page.shared_props.push(root.to_owned());
            }
            let root = prop_root(key);
            if !self
                .local_shared_roots
                .iter()
                .any(|existing| existing.as_ref() == root)
            {
                self.local_shared_roots.push(root.into());
            }
            true
        } else {
            false
        }
    }

    #[doc(hidden)]
    pub fn insert_global_shared(&mut self, key: &str, value: Value) -> bool {
        if !self.page.props.is_object() {
            self.page.props = Value::Object(Map::new());
        }
        let props = self
            .page
            .props
            .as_object_mut()
            .expect("props was normalized to an object");
        ensure_errors_prop(props);
        if insert_shared_prop_path(props, key, value) {
            let root = prop_root(key);
            if !self
                .page
                .shared_props
                .iter()
                .any(|existing| existing == root)
            {
                self.page.shared_props.push(root.to_owned());
            }
            true
        } else {
            false
        }
    }

    #[doc(hidden)]
    pub fn finish(self) -> Page<Value> {
        self.page
    }
}
