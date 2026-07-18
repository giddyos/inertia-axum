//! Synchronous lazy, optional, always, deferred, and once props.
//!
//! Entries remain in a vector to preserve insertion order and the established
//! `Vec::with_capacity`/`Map::with_capacity` allocation behavior.

use super::resolver::{IntoPageProps, PropResolver};
use crate::page::{OnceProp, PageMetadata};
use crate::request::{EffectiveRequest, RequestContext, SelectionMode, SelectionPlan};
use crate::shared::{ensure_errors_prop, prop_root};
use serde::Serialize;
use serde_json::{Map, Value};

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
            PropMode::Always => metadata.add_always(self.key.clone()),
            PropMode::Deferred { group } => {
                metadata.add_deferred(group.clone(), self.key.clone());
            }
            PropMode::Standard | PropMode::Optional => {}
        }

        if let Some((key, once)) = &self.once {
            metadata.add_once(key.clone(), once.clone());
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
            if !route_props.iter().any(|existing| existing == &route_root) {
                route_props.push(route_root);
            }

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
