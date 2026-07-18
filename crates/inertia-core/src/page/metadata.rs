//! Page-object metadata and protocol-specific merge directives.
//!
//! This public model owns deterministic `BTreeMap` metadata ordering,
//! response filtering, reset handling, and infinite-scroll merge intent.

use crate::request::RequestContext;
use crate::shared::prop_root;
use serde::Serialize;
use serde_json::Map;
use serde_json::Value;
use std::collections::BTreeMap;

fn scroll_merge_target(prop: &str) -> String {
    format!("{prop}.data")
}

fn push_unique_string(values: &mut Vec<String>, value: String) {
    if !values.contains(&value) {
        values.push(value);
    }
}

/// Additional Inertia v3 page-object metadata.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PageMetadata {
    encrypt_history: bool,
    clear_history: bool,
    preserve_fragment: bool,
    always_props: Vec<String>,
    merge_props: Vec<String>,
    prepend_props: Vec<String>,
    deep_merge_props: Vec<String>,
    match_props_on: Vec<String>,
    scroll_props: BTreeMap<String, ScrollProps>,
    deferred_props: BTreeMap<String, Vec<String>>,
    rescued_props: Vec<String>,
    shared_props: Vec<String>,
    once_props: BTreeMap<String, OnceProp>,
}

pub(crate) struct PageMetadataParts {
    pub encrypt_history: bool,
    pub clear_history: bool,
    pub preserve_fragment: bool,
    pub merge_props: Vec<String>,
    pub prepend_props: Vec<String>,
    pub deep_merge_props: Vec<String>,
    pub match_props_on: Vec<String>,
    pub scroll_props: BTreeMap<String, ScrollProps>,
    pub deferred_props: BTreeMap<String, Vec<String>>,
    pub rescued_props: Vec<String>,
    pub shared_props: Vec<String>,
    pub once_props: BTreeMap<String, OnceProp>,
}

impl PageMetadata {
    pub(crate) fn add_always(&mut self, prop: String) {
        push_unique_string(&mut self.always_props, prop);
    }
    pub(crate) fn add_deferred(&mut self, group: String, prop: String) {
        push_unique_string(self.deferred_props.entry(group).or_default(), prop);
    }
    pub(crate) fn add_once(&mut self, key: String, once: OnceProp) {
        self.once_props.insert(key, once);
    }
    pub(crate) fn into_parts(self) -> PageMetadataParts {
        PageMetadataParts {
            encrypt_history: self.encrypt_history,
            clear_history: self.clear_history,
            preserve_fragment: self.preserve_fragment,
            merge_props: self.merge_props,
            prepend_props: self.prepend_props,
            deep_merge_props: self.deep_merge_props,
            match_props_on: self.match_props_on,
            scroll_props: self.scroll_props,
            deferred_props: self.deferred_props,
            rescued_props: self.rescued_props,
            shared_props: self.shared_props,
            once_props: self.once_props,
        }
    }
    /// Creates empty page metadata.
    pub fn new() -> Self {
        Self::default()
    }

    /// Marks the page's history state for encryption.
    pub fn encrypt_history(mut self) -> Self {
        self.encrypt_history = true;
        self
    }

    /// Marks the response as clearing encrypted history state.
    pub fn clear_history(mut self) -> Self {
        self.clear_history = true;
        self
    }

    /// Preserves the original URL fragment across a redirect.
    pub fn preserve_fragment(mut self) -> Self {
        self.preserve_fragment = true;
        self
    }

    /// Marks a prop key to always be included during partial reloads.
    pub fn always<P: Into<String>>(mut self, prop: P) -> Self {
        push_unique_string(&mut self.always_props, prop.into());
        self
    }

    /// Marks a prop key for append-style merging.
    pub fn merge<P: Into<String>>(mut self, prop: P) -> Self {
        push_unique_string(&mut self.merge_props, prop.into());
        self
    }

    /// Marks a prop key for prepend-style merging.
    pub fn prepend<P: Into<String>>(mut self, prop: P) -> Self {
        push_unique_string(&mut self.prepend_props, prop.into());
        self
    }

    /// Marks a prop key for deep merging.
    pub fn deep_merge<P: Into<String>>(mut self, prop: P) -> Self {
        push_unique_string(&mut self.deep_merge_props, prop.into());
        self
    }

    /// Adds a matching key used by merge metadata.
    pub fn match_on<P: Into<String>>(mut self, prop: P) -> Self {
        push_unique_string(&mut self.match_props_on, prop.into());
        self
    }

    /// Adds infinite-scroll metadata for a prop.
    pub fn scroll<P: Into<String>>(mut self, prop: P, scroll: ScrollProps) -> Self {
        let prop = prop.into();
        let merge_target = scroll_merge_target(&prop);

        if !self.merge_props.contains(&merge_target) && !self.prepend_props.contains(&merge_target)
        {
            push_unique_string(&mut self.merge_props, merge_target);
        }

        self.scroll_props.insert(prop, scroll);
        self
    }

    /// Marks a prop as deferred in the default group.
    ///
    /// This declares page-object metadata and omits the prop value until it is
    /// explicitly requested by a partial reload. It does not install a lazy or
    /// async resolver.
    pub fn defer<P: Into<String>>(self, prop: P) -> Self {
        self.defer_group("default", prop)
    }

    /// Marks a prop as deferred in `group`.
    ///
    /// This declares page-object metadata and omits the prop value until it is
    /// explicitly requested by a partial reload. It does not install a lazy or
    /// async resolver.
    pub fn defer_group<G: Into<String>, P: Into<String>>(mut self, group: G, prop: P) -> Self {
        let props = self.deferred_props.entry(group.into()).or_default();
        push_unique_string(props, prop.into());
        self
    }

    /// Marks a deferred prop as rescued.
    ///
    /// This only serializes the `rescuedProps` metadata. It does not catch
    /// errors while resolving prop values.
    pub fn rescue<P: Into<String>>(mut self, prop: P) -> Self {
        push_unique_string(&mut self.rescued_props, prop.into());
        self
    }

    /// Marks a top-level prop as shared.
    ///
    /// This only serializes the `sharedProps` metadata. It does not register or
    /// merge global shared application state.
    pub fn share<P: Into<String>>(mut self, prop: P) -> Self {
        push_unique_string(&mut self.shared_props, prop.into());
        self
    }

    /// Marks a prop as a once prop using the prop name as the once key.
    pub fn once<P: Into<String>>(mut self, prop: P) -> Self {
        let prop = prop.into();
        self.once_props.insert(prop.clone(), OnceProp::new(prop));
        self
    }

    /// Marks a prop as a once prop with an explicit once key.
    pub fn once_with_key<K: Into<String>>(mut self, key: K, once: OnceProp) -> Self {
        self.once_props.insert(key.into(), once);
        self
    }

    /// Returns whether encrypted history is enabled.
    pub fn encrypt_history_enabled(&self) -> bool {
        self.encrypt_history
    }

    /// Returns whether clear history is enabled.
    pub fn clear_history_enabled(&self) -> bool {
        self.clear_history
    }

    /// Returns whether fragment preservation is enabled.
    pub fn preserve_fragment_enabled(&self) -> bool {
        self.preserve_fragment
    }

    /// Returns props that always survive partial-reload filtering.
    pub fn always_props(&self) -> &[String] {
        &self.always_props
    }

    /// Returns append-merge prop keys.
    pub fn merge_props(&self) -> &[String] {
        &self.merge_props
    }

    /// Returns prepend-merge prop keys.
    pub fn prepend_props(&self) -> &[String] {
        &self.prepend_props
    }

    /// Returns deep-merge prop keys.
    pub fn deep_merge_props(&self) -> &[String] {
        &self.deep_merge_props
    }

    /// Returns merge matching keys.
    pub fn match_props_on(&self) -> &[String] {
        &self.match_props_on
    }

    /// Returns infinite-scroll prop metadata.
    pub fn scroll_props(&self) -> &BTreeMap<String, ScrollProps> {
        &self.scroll_props
    }

    /// Returns deferred prop groups.
    pub fn deferred_props(&self) -> &BTreeMap<String, Vec<String>> {
        &self.deferred_props
    }

    /// Returns rescued deferred prop keys.
    pub fn rescued_props(&self) -> &[String] {
        &self.rescued_props
    }

    /// Returns shared prop keys.
    pub fn shared_props(&self) -> &[String] {
        &self.shared_props
    }

    /// Returns once-prop metadata.
    pub fn once_props(&self) -> &BTreeMap<String, OnceProp> {
        &self.once_props
    }

    pub(crate) fn into_response_metadata(
        mut self,
        context: &RequestContext,
        component: &str,
        props: Option<&Map<String, Value>>,
    ) -> Self {
        let Some(props) = props else {
            return self;
        };

        let partial_matches = context.partial_reload_matches(component);
        let included = |prop: &str| props.contains_key(prop) || props.contains_key(prop_root(prop));
        let reset_matches = |prop: &str| {
            partial_matches
                && context.reset_iter().any(|reset| {
                    prop == reset
                        || prop_root(prop) == reset
                        || prop
                            .strip_prefix(reset)
                            .is_some_and(|suffix| suffix.starts_with('.'))
                })
        };
        let once_props = &self.once_props;

        for deferred_props in self.deferred_props.values_mut() {
            deferred_props.retain(|prop| {
                !included(prop)
                    && !once_props.iter().any(|(key, once)| {
                        once.prop() == prop
                            && context
                                .except_once_props_iter()
                                .any(|excluded| excluded == key)
                    })
            });
        }
        self.deferred_props
            .retain(|_group, props| !props.is_empty());

        self.merge_props
            .retain(|prop| included(prop) && !reset_matches(prop));
        self.prepend_props
            .retain(|prop| included(prop) && !reset_matches(prop));
        self.deep_merge_props
            .retain(|prop| included(prop) && !reset_matches(prop));
        self.match_props_on
            .retain(|prop| included(prop) && !reset_matches(prop));
        self.scroll_props
            .retain(|prop, _scroll| included(prop) && !reset_matches(prop));

        if let Some(intent) = partial_matches
            .then(|| context.infinite_scroll_merge_intent())
            .flatten()
        {
            for prop in self.scroll_props.keys() {
                let target = scroll_merge_target(prop);

                if intent.eq_ignore_ascii_case("prepend") {
                    self.merge_props.retain(|prop| prop != &target);

                    if !self.prepend_props.contains(&target) {
                        self.prepend_props.push(target);
                    }
                } else if intent.eq_ignore_ascii_case("append") {
                    self.prepend_props.retain(|prop| prop != &target);

                    if !self.merge_props.contains(&target) {
                        self.merge_props.push(target);
                    }
                }
            }
        }

        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
/// Serialized pagination metadata for an infinite-scroll prop.
pub struct ScrollProps {
    page_name: String,
    previous_page: Option<u64>,
    next_page: Option<u64>,
    current_page: u64,
}

impl ScrollProps {
    /// Creates scroll metadata for `page_name` at `current_page`.
    pub fn new<P: Into<String>>(page_name: P, current_page: u64) -> Self {
        Self {
            page_name: page_name.into(),
            previous_page: None,
            next_page: None,
            current_page,
        }
    }

    /// Sets the previous page number.
    pub fn previous_page(mut self, previous_page: u64) -> Self {
        self.previous_page = Some(previous_page);
        self
    }

    /// Sets the next page number.
    pub fn next_page(mut self, next_page: u64) -> Self {
        self.next_page = Some(next_page);
        self
    }

    /// Returns the query parameter name used for pagination.
    pub fn page_name(&self) -> &str {
        &self.page_name
    }

    /// Returns the previous page number, if any.
    pub fn previous(&self) -> Option<u64> {
        self.previous_page
    }

    /// Returns the next page number, if any.
    pub fn next(&self) -> Option<u64> {
        self.next_page
    }

    /// Returns the current page number.
    pub fn current(&self) -> u64 {
        self.current_page
    }
}

/// Metadata for a prop that should only be resolved once.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnceProp {
    prop: String,
    expires_at: Option<u64>,
}

impl OnceProp {
    /// Creates once-prop metadata for `prop`.
    pub fn new<P: Into<String>>(prop: P) -> Self {
        Self {
            prop: prop.into(),
            expires_at: None,
        }
    }

    /// Sets the client-side expiration timestamp in milliseconds.
    pub fn expires_at(mut self, expires_at: u64) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    /// Returns the prop name associated with this once key.
    pub fn prop(&self) -> &str {
        &self.prop
    }

    /// Returns the optional expiration timestamp in milliseconds.
    pub fn expiration(&self) -> Option<u64> {
        self.expires_at
    }
}
