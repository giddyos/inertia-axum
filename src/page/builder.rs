use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::redirect::{Location, Redirect};
use crate::request::{EffectiveRequest, RequestContext, SelectionMode, SelectionPlan};
use crate::shared::{ensure_errors_prop, insert_shared_prop_path, prop_root};

fn is_false(value: &bool) -> bool {
    !value
}

fn empty_map<K, V>(map: &BTreeMap<K, V>) -> bool {
    map.is_empty()
}

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

impl PageMetadata {
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

    fn into_response_metadata(
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

/// Converts a value into a filtered Inertia props object.
///
/// Most callers use ordinary serializable structs or maps. [`InertiaProps`]
/// implements this trait for route-local lazy, optional, and deferred
/// synchronous resolvers.
pub trait IntoPageProps {
    /// Builds the concrete JSON props object, response metadata, and route
    /// prop roots for shared-prop collision handling.
    fn into_page_props(
        self,
        component: &str,
        request: &RequestContext,
        partial_reload_enabled: bool,
        metadata: PageMetadata,
    ) -> Result<(Value, PageMetadata, Vec<String>), serde_json::Error>;
}

impl<T: Serialize> IntoPageProps for T {
    fn into_page_props(
        self,
        component: &str,
        request: &RequestContext,
        partial_reload_enabled: bool,
        metadata: PageMetadata,
    ) -> Result<(Value, PageMetadata, Vec<String>), serde_json::Error> {
        let mut props = serde_json::to_value(self)?;
        let route_props = props
            .as_object()
            .map(|props| props.keys().cloned().collect())
            .unwrap_or_default();

        if let Some(object) = props.as_object_mut() {
            ensure_errors_prop(object);
            let plan = SelectionPlan::new(
                EffectiveRequest::new(request, partial_reload_enabled),
                component,
                &metadata,
            );
            object.retain(|key, _| key == "errors" || plan.includes(key, SelectionMode::Standard));
        }
        let metadata = metadata.into_response_metadata(request, component, props.as_object());

        Ok((props, metadata, route_props))
    }
}

trait PropResolver {
    fn resolve(self: Box<Self>) -> Result<Value, serde_json::Error>;
}

impl<F, T> PropResolver for F
where
    F: FnOnce() -> T,
    T: Serialize,
{
    fn resolve(self: Box<Self>) -> Result<Value, serde_json::Error> {
        serde_json::to_value(self())
    }
}

enum PropMode {
    Standard,
    Optional,
    Always,
    Deferred { group: String },
}

struct PropEntry<'props> {
    key: String,
    mode: PropMode,
    once: Option<(String, OnceProp)>,
    resolver: Box<dyn PropResolver + 'props>,
}

impl PropEntry<'_> {
    fn apply_metadata(&self, metadata: &mut PageMetadata) {
        match &self.mode {
            PropMode::Always => push_unique_string(&mut metadata.always_props, self.key.clone()),
            PropMode::Deferred { group } => {
                let props = metadata.deferred_props.entry(group.clone()).or_default();
                push_unique_string(props, self.key.clone());
            }
            PropMode::Standard | PropMode::Optional => {}
        }

        if let Some((key, once)) = &self.once {
            metadata.once_props.insert(key.clone(), once.clone());
        }
    }

    fn should_resolve(&self, plan: &SelectionPlan<'_, '_>) -> bool {
        plan.includes(
            &self.key,
            if matches!(self.mode, PropMode::Optional) {
                SelectionMode::Optional
            } else {
                SelectionMode::Standard
            },
        )
    }
}

/// Route-local props with synchronous lazy evaluation.
///
/// Use this when expensive props should only be serialized once an Inertia
/// request actually needs them. Standard lazy props are included on full
/// visits and matching partial reloads unless excluded. Optional props are
/// only included when explicitly requested by `X-Inertia-Partial-Data`.
/// Deferred props emit `deferredProps` metadata and are resolved when a later
/// partial reload requests them.
pub type InertiaProps = ScopedInertiaProps<'static>;

/// Lifetime-aware route-local props with synchronous lazy evaluation.
///
/// Use this variant when props are rendered immediately and resolvers need to
/// borrow data that does not live for the full `'static` lifetime. For owned
/// route return values, use [`InertiaProps`].
#[derive(Default)]
pub struct ScopedInertiaProps<'props> {
    entries: Vec<PropEntry<'props>>,
}

impl<'props> ScopedInertiaProps<'props> {
    /// Creates an empty lazy props container.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an empty lazy props container with space for `capacity` entries.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
        }
    }

    /// Adds a standard prop value.
    ///
    /// The value is already computed by the caller, but serialization is still
    /// skipped when partial reload headers omit the prop.
    pub fn value<P, T>(self, prop: P, value: T) -> Self
    where
        P: Into<String>,
        T: Serialize + 'props,
    {
        self.entry(prop, PropMode::Standard, None, move || value)
    }

    /// Adds a standard lazy prop.
    ///
    /// The resolver is called for full visits and for matching partial
    /// reloads that include the prop.
    pub fn lazy<P, F, T>(self, prop: P, resolver: F) -> Self
    where
        P: Into<String>,
        F: FnOnce() -> T + 'props,
        T: Serialize,
    {
        self.entry(prop, PropMode::Standard, None, resolver)
    }

    /// Adds a prop that is only resolved when explicitly requested.
    pub fn optional<P, F, T>(self, prop: P, resolver: F) -> Self
    where
        P: Into<String>,
        F: FnOnce() -> T + 'props,
        T: Serialize,
    {
        self.entry(prop, PropMode::Optional, None, resolver)
    }

    /// Adds a prop that is always included, even during partial reloads.
    pub fn always<P, F, T>(self, prop: P, resolver: F) -> Self
    where
        P: Into<String>,
        F: FnOnce() -> T + 'props,
        T: Serialize,
    {
        self.entry(prop, PropMode::Always, None, resolver)
    }

    /// Adds a deferred prop in the default group.
    pub fn defer<P, F, T>(self, prop: P, resolver: F) -> Self
    where
        P: Into<String>,
        F: FnOnce() -> T + 'props,
        T: Serialize,
    {
        self.defer_group("default", prop, resolver)
    }

    /// Adds a deferred prop in `group`.
    pub fn defer_group<G, P, F, T>(self, group: G, prop: P, resolver: F) -> Self
    where
        G: Into<String>,
        P: Into<String>,
        F: FnOnce() -> T + 'props,
        T: Serialize,
    {
        self.entry(
            prop,
            PropMode::Deferred {
                group: group.into(),
            },
            None,
            resolver,
        )
    }

    /// Adds a standard lazy prop with once-prop metadata.
    pub fn once<P, F, T>(self, prop: P, resolver: F) -> Self
    where
        P: Into<String>,
        F: FnOnce() -> T + 'props,
        T: Serialize,
    {
        let prop = prop.into();
        let once = OnceProp::new(prop.clone());
        self.entry(
            prop.clone(),
            PropMode::Standard,
            Some((prop, once)),
            resolver,
        )
    }

    /// Adds a standard lazy prop with a custom once key.
    pub fn once_with_key<K, F, T>(self, key: K, once: OnceProp, resolver: F) -> Self
    where
        K: Into<String>,
        F: FnOnce() -> T + 'props,
        T: Serialize,
    {
        let prop = once.prop().to_owned();
        self.entry(prop, PropMode::Standard, Some((key.into(), once)), resolver)
    }

    /// Adds an optional prop with once-prop metadata.
    pub fn optional_once<P, F, T>(self, prop: P, resolver: F) -> Self
    where
        P: Into<String>,
        F: FnOnce() -> T + 'props,
        T: Serialize,
    {
        let prop = prop.into();
        let once = OnceProp::new(prop.clone());
        self.entry(
            prop.clone(),
            PropMode::Optional,
            Some((prop, once)),
            resolver,
        )
    }

    /// Adds a deferred prop with once-prop metadata in the default group.
    pub fn defer_once<P, F, T>(self, prop: P, resolver: F) -> Self
    where
        P: Into<String>,
        F: FnOnce() -> T + 'props,
        T: Serialize,
    {
        self.defer_group_once("default", prop, resolver)
    }

    /// Adds a deferred prop with once-prop metadata in `group`.
    pub fn defer_group_once<G, P, F, T>(self, group: G, prop: P, resolver: F) -> Self
    where
        G: Into<String>,
        P: Into<String>,
        F: FnOnce() -> T + 'props,
        T: Serialize,
    {
        let prop = prop.into();
        let once = OnceProp::new(prop.clone());
        self.entry(
            prop.clone(),
            PropMode::Deferred {
                group: group.into(),
            },
            Some((prop, once)),
            resolver,
        )
    }

    fn entry<P, F, T>(
        mut self,
        prop: P,
        mode: PropMode,
        once: Option<(String, OnceProp)>,
        resolver: F,
    ) -> Self
    where
        P: Into<String>,
        F: FnOnce() -> T + 'props,
        T: Serialize,
    {
        self.entries.push(PropEntry {
            key: prop.into(),
            mode,
            once,
            resolver: Box::new(resolver),
        });
        self
    }
}

impl<'props> IntoPageProps for ScopedInertiaProps<'props> {
    fn into_page_props(
        self,
        component: &str,
        request: &RequestContext,
        partial_reload_enabled: bool,
        mut metadata: PageMetadata,
    ) -> Result<(Value, PageMetadata, Vec<String>), serde_json::Error> {
        for entry in &self.entries {
            entry.apply_metadata(&mut metadata);
        }

        let entry_count = self.entries.len();
        let mut props = Map::with_capacity(entry_count + 1);
        let mut route_props = Vec::with_capacity(entry_count);

        let plan = SelectionPlan::new(
            EffectiveRequest::new(request, partial_reload_enabled),
            component,
            &metadata,
        );
        for entry in self.entries {
            let route_root = prop_root(&entry.key).to_owned();
            push_unique_string(&mut route_props, route_root);

            if entry.should_resolve(&plan) {
                let key = entry.key;
                props.insert(key, entry.resolver.resolve()?);
            }
        }

        ensure_errors_prop(&mut props);
        let metadata = metadata.into_response_metadata(request, component, Some(&props));

        Ok((Value::Object(props), metadata, route_props))
    }
}

/// Infinite-scroll metadata for one page prop.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
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

/// A serializable Inertia page object.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    component: String,
    props: T,
    url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<Arc<str>>,
    #[serde(skip_serializing_if = "is_false")]
    encrypt_history: bool,
    #[serde(skip_serializing_if = "is_false")]
    clear_history: bool,
    #[serde(skip_serializing_if = "is_false")]
    preserve_fragment: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    merge_props: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    prepend_props: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    deep_merge_props: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    match_props_on: Vec<String>,
    #[serde(skip_serializing_if = "empty_map")]
    scroll_props: BTreeMap<String, ScrollProps>,
    #[serde(skip_serializing_if = "empty_map")]
    deferred_props: BTreeMap<String, Vec<String>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    rescued_props: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    shared_props: Vec<String>,
    #[serde(skip_serializing_if = "empty_map")]
    once_props: BTreeMap<String, OnceProp>,
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
        Self::from_parts_arc(component, props, url, version.map(Arc::from), metadata)
    }

    pub(crate) fn from_parts_arc<C: Into<String>, U: Into<String>>(
        component: C,
        props: T,
        url: U,
        version: Option<Arc<str>>,
        metadata: PageMetadata,
    ) -> Self {
        Self {
            component: component.into(),
            props,
            url: url.into(),
            version,
            encrypt_history: metadata.encrypt_history,
            clear_history: metadata.clear_history,
            preserve_fragment: metadata.preserve_fragment,
            merge_props: metadata.merge_props,
            prepend_props: metadata.prepend_props,
            deep_merge_props: metadata.deep_merge_props,
            match_props_on: metadata.match_props_on,
            scroll_props: metadata.scroll_props,
            deferred_props: metadata.deferred_props,
            rescued_props: metadata.rescued_props,
            shared_props: metadata.shared_props,
            once_props: metadata.once_props,
        }
    }

    /// Sets the page object's asset version.
    pub fn version<V: Into<String>>(mut self, version: V) -> Self {
        self.version = Some(Arc::from(version.into()));
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

    /// Returns the page URL.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Returns the asset version, if present.
    pub fn asset_version(&self) -> Option<&str> {
        self.version.as_deref()
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

/// A page under construction before shared props are merged.
pub(crate) struct PageDraft {
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

    pub(crate) fn global_is_blocked(&self, key: &str) -> bool {
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

    pub(crate) fn insert_global_shared(&mut self, key: &str, value: Value) -> bool {
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

    pub(crate) fn finish(self) -> Page<Value> {
        self.page
    }
}

/// Builder for the advanced `Inertia::page(...).props(...)` API.
pub struct InertiaPageBuilder {
    component: String,
    url: Option<String>,
    metadata: PageMetadata,
    local_shared: Vec<(String, Value)>,
}

impl InertiaPageBuilder {
    /// Marks the page's history state for encryption.
    pub fn encrypt_history(mut self) -> Self {
        self.metadata = self.metadata.encrypt_history();
        self
    }

    /// Marks the response as clearing encrypted history state.
    pub fn clear_history(mut self) -> Self {
        self.metadata = self.metadata.clear_history();
        self
    }

    /// Preserves the original URL fragment across a redirect.
    pub fn preserve_fragment(mut self) -> Self {
        self.metadata = self.metadata.preserve_fragment();
        self
    }

    /// Marks a prop key to always be included during partial reloads.
    pub fn always<P: Into<String>>(mut self, prop: P) -> Self {
        self.metadata = self.metadata.always(prop);
        self
    }

    /// Marks a prop key for append-style merging.
    pub fn merge<P: Into<String>>(mut self, prop: P) -> Self {
        self.metadata = self.metadata.merge(prop);
        self
    }

    /// Marks a prop key for prepend-style merging.
    pub fn prepend<P: Into<String>>(mut self, prop: P) -> Self {
        self.metadata = self.metadata.prepend(prop);
        self
    }

    /// Marks a prop key for deep merging.
    pub fn deep_merge<P: Into<String>>(mut self, prop: P) -> Self {
        self.metadata = self.metadata.deep_merge(prop);
        self
    }

    /// Adds a matching key used by merge metadata.
    pub fn match_on<P: Into<String>>(mut self, prop: P) -> Self {
        self.metadata = self.metadata.match_on(prop);
        self
    }

    /// Adds infinite-scroll metadata for a prop.
    pub fn scroll<P: Into<String>>(mut self, prop: P, scroll: ScrollProps) -> Self {
        self.metadata = self.metadata.scroll(prop, scroll);
        self
    }

    /// Marks a prop as deferred in the default group.
    ///
    /// This declares page-object metadata and omits the prop value until it is
    /// explicitly requested by a partial reload. It does not install a lazy or
    /// async resolver.
    pub fn defer<P: Into<String>>(mut self, prop: P) -> Self {
        self.metadata = self.metadata.defer(prop);
        self
    }

    /// Marks a prop as deferred in `group`.
    ///
    /// This declares page-object metadata and omits the prop value until it is
    /// explicitly requested by a partial reload. It does not install a lazy or
    /// async resolver.
    pub fn defer_group<G: Into<String>, P: Into<String>>(mut self, group: G, prop: P) -> Self {
        self.metadata = self.metadata.defer_group(group, prop);
        self
    }

    /// Marks a deferred prop as rescued.
    ///
    /// This only serializes the `rescuedProps` metadata. It does not catch
    /// errors while resolving prop values.
    pub fn rescue<P: Into<String>>(mut self, prop: P) -> Self {
        self.metadata = self.metadata.rescue(prop);
        self
    }

    /// Marks a top-level prop as shared.
    ///
    /// This only serializes the `sharedProps` metadata. It does not register or
    /// merge global shared application state.
    pub fn share<P: Into<String>>(mut self, prop: P) -> Self {
        self.metadata = self.metadata.share(prop);
        self
    }

    /// Marks a prop as a once prop using the prop name as the once key.
    pub fn once<P: Into<String>>(mut self, prop: P) -> Self {
        self.metadata = self.metadata.once(prop);
        self
    }

    /// Marks a prop as a once prop with an explicit once key.
    pub fn once_with_key<K: Into<String>>(mut self, key: K, once: OnceProp) -> Self {
        self.metadata = self.metadata.once_with_key(key, once);
        self
    }

    /// Sets the page props and returns an [`Inertia`] response.
    pub fn props<T>(self, props: T) -> Inertia<T> {
        Inertia {
            component: self.component,
            props,
            url: self.url,
            metadata: self.metadata,
            local_shared: self.local_shared,
        }
    }

    /// Adds a pre-serialized route-local shared value.
    pub fn shared_value<K>(mut self, key: K, value: Value) -> Self
    where
        K: Into<String>,
    {
        self.local_shared.push((key.into(), value));
        self
    }

    /// Serializes and adds a route-local shared value.
    pub fn serialize_shared<K, V>(mut self, key: K, value: V) -> Result<Self, serde_json::Error>
    where
        K: Into<String>,
        V: Serialize,
    {
        self.local_shared
            .push((key.into(), serde_json::to_value(value)?));
        Ok(self)
    }

    /// Overrides the page object's `url` field.
    pub fn with_url<U: Into<String>>(mut self, url: U) -> Self {
        self.url = Some(url.into());
        self
    }
}

impl Inertia<()> {
    /// Starts the advanced page builder API.
    pub fn page<C: Into<String>>(component: C) -> InertiaPageBuilder {
        InertiaPageBuilder {
            component: component.into(),
            url: None,
            metadata: PageMetadata::new(),
            local_shared: Vec::new(),
        }
    }

    /// Creates an external redirect response.
    ///
    /// Framework integrations should convert this into a `409 Conflict`
    /// response with the destination URL in the `X-Inertia-Location` header,
    /// or `X-Inertia-Redirect` when the destination contains a fragment.
    pub fn location<U: Into<String>>(url: U) -> Location {
        Location::new(url)
    }

    /// Creates a method-aware redirect response.
    ///
    /// Framework integrations should use `303 See Other` for write-method
    /// requests so the follow-up request is a `GET`.
    pub fn redirect<U: Into<String>>(url: U) -> Redirect {
        Redirect::new(url)
    }
}

/// A framework-neutral Inertia page response.
///
/// Framework integrations convert this value into either an HTML first-load
/// response or a JSON Inertia response, depending on the incoming request
/// headers.
pub struct Inertia<T> {
    component: String,
    props: T,
    url: Option<String>,
    metadata: PageMetadata,
    local_shared: Vec<(String, Value)>,
}

impl<T> Inertia<T> {
    /// Constructs a response for `component` with serializable `props`.
    ///
    /// Framework integrations default the page object's `url` field to the
    /// current request URI unless [`with_url`](Self::with_url) is used.
    pub fn response<C: Into<String>>(component: C, props: T) -> Self {
        Self {
            component: component.into(),
            props,
            url: None,
            metadata: PageMetadata::new(),
            local_shared: Vec::new(),
        }
    }

    /// Overrides the page object's `url` field.
    pub fn with_url<U: Into<String>>(mut self, url: U) -> Self {
        self.url = Some(url.into());
        self
    }

    /// Marks the page's history state for encryption.
    pub fn encrypt_history(mut self) -> Self {
        self.metadata = self.metadata.encrypt_history();
        self
    }

    /// Marks the response as clearing encrypted history state.
    pub fn clear_history(mut self) -> Self {
        self.metadata = self.metadata.clear_history();
        self
    }

    /// Preserves the original URL fragment across a redirect.
    pub fn preserve_fragment(mut self) -> Self {
        self.metadata = self.metadata.preserve_fragment();
        self
    }

    /// Marks a prop key to always be included during partial reloads.
    pub fn always<P: Into<String>>(mut self, prop: P) -> Self {
        self.metadata = self.metadata.always(prop);
        self
    }

    /// Marks a prop key for append-style merging.
    pub fn merge<P: Into<String>>(mut self, prop: P) -> Self {
        self.metadata = self.metadata.merge(prop);
        self
    }

    /// Marks a prop key for prepend-style merging.
    pub fn prepend<P: Into<String>>(mut self, prop: P) -> Self {
        self.metadata = self.metadata.prepend(prop);
        self
    }

    /// Marks a prop key for deep merging.
    pub fn deep_merge<P: Into<String>>(mut self, prop: P) -> Self {
        self.metadata = self.metadata.deep_merge(prop);
        self
    }

    /// Adds a matching key used by merge metadata.
    pub fn match_on<P: Into<String>>(mut self, prop: P) -> Self {
        self.metadata = self.metadata.match_on(prop);
        self
    }

    /// Adds infinite-scroll metadata for a prop.
    pub fn scroll<P: Into<String>>(mut self, prop: P, scroll: ScrollProps) -> Self {
        self.metadata = self.metadata.scroll(prop, scroll);
        self
    }

    /// Marks a prop as deferred in the default group.
    ///
    /// This declares page-object metadata and omits the prop value until it is
    /// explicitly requested by a partial reload. It does not install a lazy or
    /// async resolver.
    pub fn defer<P: Into<String>>(mut self, prop: P) -> Self {
        self.metadata = self.metadata.defer(prop);
        self
    }

    /// Marks a prop as deferred in `group`.
    ///
    /// This declares page-object metadata and omits the prop value until it is
    /// explicitly requested by a partial reload. It does not install a lazy or
    /// async resolver.
    pub fn defer_group<G: Into<String>, P: Into<String>>(mut self, group: G, prop: P) -> Self {
        self.metadata = self.metadata.defer_group(group, prop);
        self
    }

    /// Marks a deferred prop as rescued.
    ///
    /// This only serializes the `rescuedProps` metadata. It does not catch
    /// errors while resolving prop values.
    pub fn rescue<P: Into<String>>(mut self, prop: P) -> Self {
        self.metadata = self.metadata.rescue(prop);
        self
    }

    /// Marks a top-level prop as shared.
    ///
    /// This only serializes the `sharedProps` metadata. It does not register or
    /// merge global shared application state.
    pub fn share<P: Into<String>>(mut self, prop: P) -> Self {
        self.metadata = self.metadata.share(prop);
        self
    }

    /// Marks a prop as a once prop using the prop name as the once key.
    pub fn once<P: Into<String>>(mut self, prop: P) -> Self {
        self.metadata = self.metadata.once(prop);
        self
    }

    /// Marks a prop as a once prop with an explicit once key.
    pub fn once_with_key<K: Into<String>>(mut self, key: K, once: OnceProp) -> Self {
        self.metadata = self.metadata.once_with_key(key, once);
        self
    }

    /// Returns the Inertia component name.
    pub fn component(&self) -> &str {
        &self.component
    }

    /// Returns a reference to the component props.
    pub fn props(&self) -> &T {
        &self.props
    }

    /// Returns the explicit page URL override, if one was set.
    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }

    /// Returns the configured page metadata.
    pub fn metadata(&self) -> &PageMetadata {
        &self.metadata
    }

    /// Adds a pre-serialized route-local shared value.
    pub fn shared_value<K>(mut self, key: K, value: Value) -> Self
    where
        K: Into<String>,
    {
        self.local_shared.push((key.into(), value));
        self
    }

    /// Serializes and adds a route-local shared value.
    pub fn serialize_shared<K, V>(mut self, key: K, value: V) -> Result<Self, serde_json::Error>
    where
        K: Into<String>,
        V: Serialize,
    {
        self.local_shared
            .push((key.into(), serde_json::to_value(value)?));
        Ok(self)
    }
}

impl<T: IntoPageProps> Inertia<T> {
    pub(crate) fn into_page_draft(
        self,
        default_url: &str,
        version: Option<Arc<str>>,
        request: &RequestContext,
        partial_reload_enabled: bool,
    ) -> Result<PageDraft, serde_json::Error> {
        let component = self.component;
        let url = self.url.unwrap_or_else(|| default_url.to_owned());
        let (props, metadata, route_props) = self.props.into_page_props(
            &component,
            request,
            partial_reload_enabled,
            self.metadata,
        )?;
        let mut draft = PageDraft::new(
            Page::from_parts_arc(component, props, url, version, metadata),
            route_props,
        );
        for (key, value) in self.local_shared {
            draft.insert_shared(&key, value);
        }
        Ok(draft)
    }

    /// Builds a concrete Inertia page object.
    ///
    /// Framework integrations pass the resolved request URL, asset version,
    /// and parsed request context so props can be filtered for partial reloads,
    /// deferred props, and once props.
    pub fn into_page(
        self,
        url: impl Into<String>,
        version: Option<String>,
        request: &RequestContext,
    ) -> Result<Page<Value>, serde_json::Error> {
        let url = url.into();
        self.into_page_draft(&url, version.map(Arc::from), request, true)
            .map(PageDraft::finish)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;
    use serde_json::json;
    use std::cell::Cell;
    use std::collections::HashMap;
    use std::rc::Rc;

    fn request_context_from(headers: &[(&str, &str)]) -> RequestContext {
        let headers = headers.iter().copied().collect::<HashMap<_, _>>();

        RequestContext::from_header_fn(|name| headers.get(name).copied())
    }

    #[test]
    fn request_context_parses_inertia_headers() {
        let context = request_context_from(&[
            (X_INERTIA, "true"),
            (X_INERTIA_VERSION, "abc"),
            (X_INERTIA_PARTIAL_COMPONENT, "Users/Index"),
            (X_INERTIA_PARTIAL_DATA, "users, stats"),
            (X_INERTIA_RESET, "users"),
            (X_INERTIA_ERROR_BAG, "createUser"),
            (X_INERTIA_INFINITE_SCROLL_MERGE_INTENT, "append"),
            (X_INERTIA_EXCEPT_ONCE_PROPS, "plans,features"),
            (PURPOSE, "prefetch"),
            (CACHE_CONTROL, "max-age=0, no-cache"),
        ]);

        assert!(context.is_inertia());
        assert_eq!(context.version(), Some("abc"));
        assert_eq!(context.partial_component(), Some("Users/Index"));
        assert_eq!(context.partial_data(), ["users", "stats"]);
        assert_eq!(context.reset(), ["users"]);
        assert_eq!(context.error_bag(), Some("createUser"));
        assert_eq!(context.infinite_scroll_merge_intent(), Some("append"));
        assert_eq!(context.except_once_props(), ["plans", "features"]);
        assert!(context.is_prefetch());
        assert!(context.is_reload());
    }

    #[test]
    fn page_serializes_v3_metadata() {
        let page = Page::from_parts(
            "Feed/Index",
            json!({ "errors": {}, "posts": [{ "id": 1 }] }),
            "/feed",
            Some("version-1".into()),
            PageMetadata::new()
                .encrypt_history()
                .clear_history()
                .preserve_fragment()
                .merge("posts")
                .prepend("notifications")
                .deep_merge("conversations")
                .match_on("posts.id")
                .scroll("posts", ScrollProps::new("page", 1).next_page(2))
                .defer("analytics")
                .rescue("analytics")
                .share("auth")
                .once("plans"),
        );

        let value = serde_json::to_value(page).unwrap();

        assert_eq!(value["component"], "Feed/Index");
        assert_eq!(value["version"], "version-1");
        assert_eq!(value["encryptHistory"], true);
        assert_eq!(value["clearHistory"], true);
        assert_eq!(value["preserveFragment"], true);
        assert_eq!(value["mergeProps"], json!(["posts", "posts.data"]));
        assert_eq!(value["prependProps"], json!(["notifications"]));
        assert_eq!(value["deepMergeProps"], json!(["conversations"]));
        assert_eq!(value["matchPropsOn"], json!(["posts.id"]));
        assert_eq!(value["scrollProps"]["posts"]["pageName"], "page");
        assert_eq!(value["scrollProps"]["posts"]["nextPage"], 2);
        assert_eq!(value["deferredProps"], json!({ "default": ["analytics"] }));
        assert_eq!(value["rescuedProps"], json!(["analytics"]));
        assert_eq!(value["sharedProps"], json!(["auth"]));
        assert_eq!(
            value["onceProps"]["plans"],
            json!({ "prop": "plans", "expiresAt": null })
        );
    }

    #[test]
    fn route_local_shared_values_merge_before_global_values() {
        let page = Inertia::response("Dashboard", serde_json::json!({}))
            .shared_value("auth.user", serde_json::json!({"name": "Route"}))
            .into_page("/dashboard", None, &RequestContext::default())
            .unwrap()
            .with_shared_props([("auth.user", serde_json::json!({"name": "Global"}))]);

        assert_eq!(page.props()["auth"]["user"]["name"], "Route");
    }

    #[test]
    fn partial_data_filters_matching_component_props() {
        let context = request_context_from(&[
            (X_INERTIA, "true"),
            (X_INERTIA_PARTIAL_COMPONENT, "Events"),
            (X_INERTIA_PARTIAL_DATA, "events"),
        ]);
        let mut props = json!({
            "auth": { "name": "Ada" },
            "events": [1, 2],
            "categories": ["meetups"]
        });

        context.filter_props("Events", &mut props, &PageMetadata::new().always("auth"));

        assert_eq!(
            props,
            json!({
                "errors": {},
                "auth": { "name": "Ada" },
                "events": [1, 2]
            })
        );
    }

    #[test]
    fn partial_except_takes_precedence_over_partial_data() {
        let context = request_context_from(&[
            (X_INERTIA, "true"),
            (X_INERTIA_PARTIAL_COMPONENT, "Events"),
            (X_INERTIA_PARTIAL_DATA, "events"),
            (X_INERTIA_PARTIAL_EXCEPT, "categories"),
        ]);
        let mut props = json!({
            "auth": { "name": "Ada" },
            "events": [1, 2],
            "categories": ["meetups"]
        });

        context.filter_props("Events", &mut props, &PageMetadata::new());

        assert_eq!(
            props,
            json!({
                "errors": {},
                "auth": { "name": "Ada" },
                "events": [1, 2]
            })
        );
    }

    #[test]
    fn partial_except_without_partial_data_excludes_listed_props() {
        let context = request_context_from(&[
            (X_INERTIA, "true"),
            (X_INERTIA_PARTIAL_COMPONENT, "Events"),
            (X_INERTIA_PARTIAL_EXCEPT, "categories"),
        ]);
        let mut props = json!({
            "events": [1, 2],
            "categories": ["meetups"],
            "filters": { "open": true }
        });

        context.filter_props("Events", &mut props, &PageMetadata::new());

        assert_eq!(
            props,
            json!({
                "errors": {},
                "events": [1, 2],
                "filters": { "open": true }
            })
        );
    }

    #[test]
    fn partial_headers_are_ignored_for_different_components() {
        let context = request_context_from(&[
            (X_INERTIA, "true"),
            (X_INERTIA_PARTIAL_COMPONENT, "Events"),
            (X_INERTIA_PARTIAL_DATA, "events"),
        ]);
        let mut props = json!({
            "auth": { "name": "Ada" },
            "events": [1, 2]
        });

        context.filter_props("Dashboard", &mut props, &PageMetadata::new());

        assert_eq!(
            props,
            json!({
                "errors": {},
                "auth": { "name": "Ada" },
                "events": [1, 2]
            })
        );
    }

    #[test]
    fn deferred_and_once_props_are_omitted_until_explicitly_requested() {
        let context =
            request_context_from(&[(X_INERTIA, "true"), (X_INERTIA_EXCEPT_ONCE_PROPS, "plans")]);
        let mut props = json!({
            "analytics": { "views": 10 },
            "plans": ["basic"],
            "user": { "name": "Ada" }
        });
        let metadata = PageMetadata::new().defer("analytics").once("plans");

        context.filter_props("Dashboard", &mut props, &metadata);

        assert_eq!(
            props,
            json!({
                "errors": {},
                "user": { "name": "Ada" }
            })
        );

        let context = request_context_from(&[
            (X_INERTIA, "true"),
            (X_INERTIA_PARTIAL_COMPONENT, "Dashboard"),
            (X_INERTIA_PARTIAL_DATA, "analytics,plans"),
            (X_INERTIA_EXCEPT_ONCE_PROPS, "plans"),
        ]);
        let mut props = json!({
            "analytics": { "views": 10 },
            "plans": ["basic"],
            "user": { "name": "Ada" }
        });

        context.filter_props("Dashboard", &mut props, &metadata);

        assert_eq!(
            props,
            json!({
                "analytics": { "views": 10 },
                "errors": {},
                "plans": ["basic"]
            })
        );
    }

    #[test]
    fn request_reset_filters_merge_and_scroll_metadata() {
        let request = request_context_from(&[
            (X_INERTIA, "true"),
            (X_INERTIA_PARTIAL_COMPONENT, "Feed"),
            (X_INERTIA_PARTIAL_DATA, "posts"),
            (X_INERTIA_RESET, "posts"),
        ]);
        let response = Inertia::page("Feed")
            .scroll("posts", ScrollProps::new("page", 1).next_page(2))
            .match_on("posts.data.id")
            .props(json!({ "posts": { "data": [1, 2] } }))
            .into_page("/feed", Some("version-1".into()), &request)
            .unwrap();
        let value = serde_json::to_value(response).unwrap();

        assert_eq!(value["props"]["posts"]["data"], json!([1, 2]));
        assert!(value.get("mergeProps").is_none());
        assert!(value.get("matchPropsOn").is_none());
        assert!(value.get("scrollProps").is_none());
    }

    #[test]
    fn reset_and_scroll_intent_are_ignored_when_partial_component_differs() {
        let request = request_context_from(&[
            (X_INERTIA, "true"),
            (X_INERTIA_PARTIAL_COMPONENT, "Other"),
            (X_INERTIA_PARTIAL_DATA, "posts"),
            (X_INERTIA_RESET, "posts"),
            (X_INERTIA_INFINITE_SCROLL_MERGE_INTENT, "prepend"),
        ]);
        let response = Inertia::page("Feed")
            .scroll("posts", ScrollProps::new("page", 1).next_page(2))
            .props(json!({ "posts": { "data": [1, 2] } }))
            .into_page("/feed", Some("version-1".into()), &request)
            .unwrap();
        let value = serde_json::to_value(response).unwrap();

        assert_eq!(value["props"]["posts"]["data"], json!([1, 2]));
        assert_eq!(value["mergeProps"], json!(["posts.data"]));
        assert_eq!(value["scrollProps"]["posts"]["nextPage"], 2);
        assert!(value.get("prependProps").is_none());
    }

    #[test]
    fn infinite_scroll_merge_intent_can_prepend_scroll_props() {
        let request = request_context_from(&[
            (X_INERTIA, "true"),
            (X_INERTIA_PARTIAL_COMPONENT, "Feed"),
            (X_INERTIA_PARTIAL_DATA, "posts"),
            (X_INERTIA_INFINITE_SCROLL_MERGE_INTENT, "prepend"),
        ]);
        let response = Inertia::page("Feed")
            .scroll("posts", ScrollProps::new("page", 1).next_page(2))
            .props(json!({ "posts": { "data": [1, 2] } }))
            .into_page("/feed", Some("version-1".into()), &request)
            .unwrap();
        let value = serde_json::to_value(response).unwrap();

        assert_eq!(value["prependProps"], json!(["posts.data"]));
        assert!(value.get("mergeProps").is_none());
        assert_eq!(value["scrollProps"]["posts"]["nextPage"], 2);
    }

    #[test]
    fn once_with_custom_key_omits_loaded_prop_until_requested() {
        let request = request_context_from(&[
            (X_INERTIA, "true"),
            (X_INERTIA_EXCEPT_ONCE_PROPS, "billing"),
        ]);
        let response = Inertia::response(
            "Billing",
            json!({
                "current_plan": "basic",
                "plans": ["basic", "pro"]
            }),
        )
        .once_with_key("billing", OnceProp::new("plans").expires_at(123))
        .into_page("/billing", Some("version-1".into()), &request)
        .unwrap();
        let value = serde_json::to_value(response).unwrap();

        assert!(value["props"].get("plans").is_none());
        assert_eq!(
            value["onceProps"]["billing"],
            json!({ "prop": "plans", "expiresAt": 123 })
        );
    }

    #[test]
    fn lazy_props_are_only_resolved_when_included() {
        let request = request_context_from(&[]);
        let calls = Rc::new(Cell::new(0));
        let response = Inertia::response(
            "Dashboard",
            InertiaProps::new()
                .value("user", json!({ "name": "Ada" }))
                .lazy("stats", {
                    let calls = Rc::clone(&calls);
                    move || {
                        calls.set(calls.get() + 1);
                        json!({ "views": 10 })
                    }
                }),
        )
        .into_page("/dashboard", Some("version-1".into()), &request)
        .unwrap();
        let value = serde_json::to_value(response).unwrap();

        assert_eq!(calls.get(), 1);
        assert_eq!(value["props"]["stats"]["views"], 10);

        let request = request_context_from(&[
            (X_INERTIA, "true"),
            (X_INERTIA_PARTIAL_COMPONENT, "Dashboard"),
            (X_INERTIA_PARTIAL_DATA, "user"),
        ]);
        let calls = Rc::new(Cell::new(0));
        let response = Inertia::response(
            "Dashboard",
            InertiaProps::new()
                .value("user", json!({ "name": "Ada" }))
                .lazy("stats", {
                    let calls = Rc::clone(&calls);
                    move || {
                        calls.set(calls.get() + 1);
                        json!({ "views": 10 })
                    }
                }),
        )
        .into_page("/dashboard", Some("version-1".into()), &request)
        .unwrap();
        let value = serde_json::to_value(response).unwrap();

        assert_eq!(calls.get(), 0);
        assert_eq!(value["props"]["user"]["name"], "Ada");
        assert!(value["props"].get("stats").is_none());
    }

    #[test]
    fn lazy_props_can_borrow_values_for_immediate_rendering() {
        let request = request_context_from(&[]);
        let name = String::from("Ada");
        let response = Inertia::response(
            "Profile",
            ScopedInertiaProps::new()
                .value("name", &name)
                .lazy("upperName", || name.to_uppercase()),
        )
        .into_page("/profile", Some("version-1".into()), &request)
        .unwrap();
        let value = serde_json::to_value(response).unwrap();

        assert_eq!(value["props"]["name"], "Ada");
        assert_eq!(value["props"]["upperName"], "ADA");
    }

    #[test]
    fn optional_props_resolve_only_when_explicitly_requested() {
        let request = request_context_from(&[]);
        let calls = Rc::new(Cell::new(0));
        let response = Inertia::response(
            "Dashboard",
            InertiaProps::new().optional("audit", {
                let calls = Rc::clone(&calls);
                move || {
                    calls.set(calls.get() + 1);
                    json!(["created"])
                }
            }),
        )
        .into_page("/dashboard", Some("version-1".into()), &request)
        .unwrap();
        let value = serde_json::to_value(response).unwrap();

        assert_eq!(calls.get(), 0);
        assert!(value["props"].get("audit").is_none());

        let request = request_context_from(&[
            (X_INERTIA, "true"),
            (X_INERTIA_PARTIAL_COMPONENT, "Dashboard"),
            (X_INERTIA_PARTIAL_DATA, "audit"),
        ]);
        let calls = Rc::new(Cell::new(0));
        let response = Inertia::response(
            "Dashboard",
            InertiaProps::new().optional("audit", {
                let calls = Rc::clone(&calls);
                move || {
                    calls.set(calls.get() + 1);
                    json!(["created"])
                }
            }),
        )
        .into_page("/dashboard", Some("version-1".into()), &request)
        .unwrap();
        let value = serde_json::to_value(response).unwrap();

        assert_eq!(calls.get(), 1);
        assert_eq!(value["props"]["audit"], json!(["created"]));
    }

    #[test]
    fn optional_props_respect_partial_except_precedence() {
        let request = request_context_from(&[
            (X_INERTIA, "true"),
            (X_INERTIA_PARTIAL_COMPONENT, "Dashboard"),
            (X_INERTIA_PARTIAL_DATA, "audit"),
            (X_INERTIA_PARTIAL_EXCEPT, "audit"),
        ]);
        let calls = Rc::new(Cell::new(0));
        let response = Inertia::response(
            "Dashboard",
            InertiaProps::new().optional("audit", {
                let calls = Rc::clone(&calls);
                move || {
                    calls.set(calls.get() + 1);
                    json!(["created"])
                }
            }),
        )
        .into_page("/dashboard", Some("version-1".into()), &request)
        .unwrap();
        let value = serde_json::to_value(response).unwrap();

        assert_eq!(calls.get(), 0);
        assert!(value["props"].get("audit").is_none());
    }

    #[test]
    fn lazy_errors_are_preserved_during_partial_reloads() {
        let request = request_context_from(&[
            (X_INERTIA, "true"),
            (X_INERTIA_PARTIAL_COMPONENT, "Form"),
            (X_INERTIA_PARTIAL_DATA, "user"),
        ]);
        let calls = Rc::new(Cell::new(0));
        let response = Inertia::response(
            "Form",
            InertiaProps::new()
                .value("user", json!({ "name": "Ada" }))
                .lazy("errors", {
                    let calls = Rc::clone(&calls);
                    move || {
                        calls.set(calls.get() + 1);
                        json!({ "name": "Required" })
                    }
                })
                .lazy("stats", || 10),
        )
        .into_page("/form", Some("version-1".into()), &request)
        .unwrap();
        let value = serde_json::to_value(response).unwrap();

        assert_eq!(calls.get(), 1);
        assert_eq!(value["props"]["user"]["name"], "Ada");
        assert_eq!(value["props"]["errors"]["name"], "Required");
        assert!(value["props"].get("stats").is_none());
    }

    #[test]
    fn deferred_props_emit_metadata_and_resolve_only_when_requested() {
        let request = request_context_from(&[]);
        let calls = Rc::new(Cell::new(0));
        let response = Inertia::response(
            "Dashboard",
            InertiaProps::new().defer_group("metrics", "analytics", {
                let calls = Rc::clone(&calls);
                move || {
                    calls.set(calls.get() + 1);
                    json!({ "views": 10 })
                }
            }),
        )
        .into_page("/dashboard", Some("version-1".into()), &request)
        .unwrap();
        let value = serde_json::to_value(response).unwrap();

        assert_eq!(calls.get(), 0);
        assert!(value["props"].get("analytics").is_none());
        assert_eq!(value["deferredProps"], json!({ "metrics": ["analytics"] }));

        let request = request_context_from(&[
            (X_INERTIA, "true"),
            (X_INERTIA_PARTIAL_COMPONENT, "Dashboard"),
            (X_INERTIA_PARTIAL_DATA, "analytics"),
        ]);
        let calls = Rc::new(Cell::new(0));
        let response = Inertia::response(
            "Dashboard",
            InertiaProps::new().defer_group("metrics", "analytics", {
                let calls = Rc::clone(&calls);
                move || {
                    calls.set(calls.get() + 1);
                    json!({ "views": 10 })
                }
            }),
        )
        .into_page("/dashboard", Some("version-1".into()), &request)
        .unwrap();
        let value = serde_json::to_value(response).unwrap();

        assert_eq!(calls.get(), 1);
        assert_eq!(value["props"]["analytics"]["views"], 10);
        assert!(value.get("deferredProps").is_none());
    }

    #[test]
    fn deferred_once_props_already_loaded_by_client_are_not_advertised() {
        let request =
            request_context_from(&[(X_INERTIA, "true"), (X_INERTIA_EXCEPT_ONCE_PROPS, "stats")]);
        let calls = Rc::new(Cell::new(0));
        let response = Inertia::response(
            "Dashboard",
            InertiaProps::new().defer_once("stats", {
                let calls = Rc::clone(&calls);
                move || {
                    calls.set(calls.get() + 1);
                    10
                }
            }),
        )
        .into_page("/dashboard", Some("version-1".into()), &request)
        .unwrap();
        let value = serde_json::to_value(response).unwrap();

        assert_eq!(calls.get(), 0);
        assert!(value["props"].get("stats").is_none());
        assert!(value.get("deferredProps").is_none());
        assert_eq!(
            value["onceProps"]["stats"],
            json!({ "prop": "stats", "expiresAt": null })
        );
    }

    #[test]
    fn always_lazy_props_survive_partial_reload_filtering() {
        let request = request_context_from(&[
            (X_INERTIA, "true"),
            (X_INERTIA_PARTIAL_COMPONENT, "Dashboard"),
            (X_INERTIA_PARTIAL_DATA, "users"),
        ]);
        let calls = Rc::new(Cell::new(0));
        let response = Inertia::response(
            "Dashboard",
            InertiaProps::new()
                .value("users", json!(["Ada"]))
                .always("auth", {
                    let calls = Rc::clone(&calls);
                    move || {
                        calls.set(calls.get() + 1);
                        json!({ "user": { "name": "Ada" } })
                    }
                }),
        )
        .into_page("/dashboard", Some("version-1".into()), &request)
        .unwrap();
        let value = serde_json::to_value(response).unwrap();

        assert_eq!(calls.get(), 1);
        assert_eq!(value["props"]["users"], json!(["Ada"]));
        assert_eq!(value["props"]["auth"]["user"]["name"], "Ada");
    }

    #[test]
    fn once_lazy_props_are_not_resolved_when_client_already_has_them() {
        let request =
            request_context_from(&[(X_INERTIA, "true"), (X_INERTIA_EXCEPT_ONCE_PROPS, "plans")]);
        let calls = Rc::new(Cell::new(0));
        let response = Inertia::response(
            "Billing",
            InertiaProps::new().once("plans", {
                let calls = Rc::clone(&calls);
                move || {
                    calls.set(calls.get() + 1);
                    json!(["basic"])
                }
            }),
        )
        .into_page("/billing", Some("version-1".into()), &request)
        .unwrap();
        let value = serde_json::to_value(response).unwrap();

        assert_eq!(calls.get(), 0);
        assert!(value["props"].get("plans").is_none());
        assert_eq!(
            value["onceProps"]["plans"],
            json!({ "prop": "plans", "expiresAt": null })
        );

        let request = request_context_from(&[
            (X_INERTIA, "true"),
            (X_INERTIA_PARTIAL_COMPONENT, "Billing"),
            (X_INERTIA_PARTIAL_DATA, "plans"),
            (X_INERTIA_EXCEPT_ONCE_PROPS, "plans"),
        ]);
        let calls = Rc::new(Cell::new(0));
        let response = Inertia::response(
            "Billing",
            InertiaProps::new().once("plans", {
                let calls = Rc::clone(&calls);
                move || {
                    calls.set(calls.get() + 1);
                    json!(["basic"])
                }
            }),
        )
        .into_page("/billing", Some("version-1".into()), &request)
        .unwrap();
        let value = serde_json::to_value(response).unwrap();

        assert_eq!(calls.get(), 1);
        assert_eq!(value["props"]["plans"], json!(["basic"]));
    }

    #[test]
    fn lazy_route_prop_roots_block_shared_props_even_when_omitted() {
        let request = request_context_from(&[]);
        let response = Inertia::response(
            "Dashboard",
            InertiaProps::new().optional("auth", || json!({ "user": { "name": "Route" } })),
        )
        .into_page("/dashboard", Some("version-1".into()), &request)
        .unwrap()
        .with_shared_props(vec![
            (
                "auth.user",
                json!({
                    "name": "Shared"
                }),
            ),
            ("appName", json!("Demo")),
        ]);
        let value = serde_json::to_value(response).unwrap();

        assert_eq!(value["props"]["auth"]["user"]["name"], "Shared");
        assert_eq!(value["props"]["appName"], "Demo");
        assert_eq!(value["sharedProps"], json!(["auth", "appName"]));
    }

    #[test]
    fn empty_shared_props_are_a_noop() {
        let page = Page::new("Empty", Value::Null, "/empty")
            .with_shared_props(Vec::<(&str, Value)>::new());
        let value = serde_json::to_value(page).unwrap();

        assert_eq!(value["props"], Value::Null);
        assert!(value.get("sharedProps").is_none());
    }

    #[test]
    fn page_equality_ignores_internal_route_prop_tracking() {
        let request = request_context_from(&[]);
        let response = Inertia::response(
            "Users",
            json!({
                "auth": {
                    "user": {
                        "name": "Ada"
                    }
                }
            }),
        )
        .into_page("/users", Some("version-1".into()), &request)
        .unwrap();
        let manual = Page::from_parts(
            "Users",
            json!({
                "errors": {},
                "auth": {
                    "user": {
                        "name": "Ada"
                    }
                }
            }),
            "/users",
            Some("version-1".into()),
            PageMetadata::new(),
        );

        assert_eq!(response, manual);
    }
}
