//! Parsed Inertia request headers and partial-reload accessors.
//!
//! This module is consumed by prop selection and framework adapters. Header
//! values remain boxed strings and token accessors iterate comma-separated
//! values without allocation; only the existing `Vec<String>` accessors copy.

use super::header_list::HeaderList;
use crate::headers::*;
use crate::page::PageMetadata;
use crate::request::{EffectiveRequest, SelectionMode, SelectionPlan};
use crate::shared::ensure_errors_prop;
use serde_json::Value;

fn contains_no_cache(value: &str) -> bool {
    value
        .split(',')
        .map(str::trim)
        .any(|part| part.eq_ignore_ascii_case("no-cache"))
}

impl RequestContext {
    pub(crate) fn partial_data_contains(&self, key: &str) -> bool {
        self.partial_data.contains(key)
    }

    pub(crate) fn partial_data_is_empty(&self) -> bool {
        self.partial_data.is_empty()
    }

    pub(crate) fn partial_except_contains(&self, key: &str) -> bool {
        self.partial_except.contains(key)
    }

    pub(crate) fn partial_except_is_empty(&self) -> bool {
        self.partial_except.is_empty()
    }

    pub(crate) fn once_prop_is_excluded(&self, key: &str) -> bool {
        self.except_once_props.contains(key)
    }
}

/// Parsed Inertia request headers.
///
/// Framework integrations use this type to avoid duplicating header parsing
/// and partial-reload semantics.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RequestContext {
    is_inertia: bool,
    version: Option<Box<str>>,
    partial_component: Option<Box<str>>,
    partial_data: HeaderList,
    partial_except: HeaderList,
    reset: HeaderList,
    error_bag: Option<Box<str>>,
    infinite_scroll_merge_intent: Option<Box<str>>,
    except_once_props: HeaderList,
    is_prefetch: bool,
    is_reload: bool,
}

impl RequestContext {
    /// Parses a request context from a function that returns header values.
    ///
    /// The callback is invoked with canonical protocol header names such as
    /// `X-Inertia-Partial-Data`. HTTP frameworks usually handle
    /// case-insensitive lookup at this boundary.
    pub fn from_header_fn<'a, F>(mut header: F) -> Self
    where
        F: FnMut(&str) -> Option<&'a str>,
    {
        Self {
            is_inertia: header(X_INERTIA).is_some(),
            version: header(X_INERTIA_VERSION).map(Into::into),
            partial_component: header(X_INERTIA_PARTIAL_COMPONENT).map(Into::into),
            partial_data: HeaderList::parse(header(X_INERTIA_PARTIAL_DATA)),
            partial_except: HeaderList::parse(header(X_INERTIA_PARTIAL_EXCEPT)),
            reset: HeaderList::parse(header(X_INERTIA_RESET)),
            error_bag: header(X_INERTIA_ERROR_BAG).map(Into::into),
            infinite_scroll_merge_intent: header(X_INERTIA_INFINITE_SCROLL_MERGE_INTENT)
                .map(Into::into),
            except_once_props: HeaderList::parse(header(X_INERTIA_EXCEPT_ONCE_PROPS)),
            is_prefetch: header(PURPOSE)
                .map(|purpose| purpose.eq_ignore_ascii_case("prefetch"))
                .unwrap_or(false),
            is_reload: header(CACHE_CONTROL)
                .map(contains_no_cache)
                .unwrap_or(false),
        }
    }

    /// Returns `true` when the request includes the `X-Inertia` header.
    pub fn is_inertia(&self) -> bool {
        self.is_inertia
    }

    /// Returns the request's `X-Inertia-Version` header value.
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    /// Returns the partial reload component, if present.
    pub fn partial_component(&self) -> Option<&str> {
        self.partial_component.as_deref()
    }

    /// Returns the props requested through `X-Inertia-Partial-Data`.
    pub fn partial_data(&self) -> Vec<String> {
        self.partial_data.iter().map(ToOwned::to_owned).collect()
    }

    /// Iterates partial-data prop names without allocating.
    pub fn partial_data_iter(&self) -> impl Iterator<Item = &str> + '_ {
        self.partial_data.iter()
    }

    /// Returns the props excluded through `X-Inertia-Partial-Except`.
    pub fn partial_except(&self) -> Vec<String> {
        self.partial_except.iter().map(ToOwned::to_owned).collect()
    }

    /// Iterates partial-except prop names without allocating.
    pub fn partial_except_iter(&self) -> impl Iterator<Item = &str> + '_ {
        self.partial_except.iter()
    }

    /// Returns the props requested for reset on navigation.
    pub fn reset(&self) -> Vec<String> {
        self.reset.iter().map(ToOwned::to_owned).collect()
    }

    /// Iterates reset prop names without allocating.
    pub fn reset_iter(&self) -> impl Iterator<Item = &str> + '_ {
        self.reset.iter()
    }

    /// Returns the validation error bag name, if present.
    pub fn error_bag(&self) -> Option<&str> {
        self.error_bag.as_deref()
    }

    /// Returns the infinite-scroll merge intent, if present.
    pub fn infinite_scroll_merge_intent(&self) -> Option<&str> {
        self.infinite_scroll_merge_intent.as_deref()
    }

    /// Returns once-prop keys the client already has.
    pub fn except_once_props(&self) -> Vec<String> {
        self.except_once_props
            .iter()
            .map(ToOwned::to_owned)
            .collect()
    }

    /// Iterates once-prop keys without allocating.
    pub fn except_once_props_iter(&self) -> impl Iterator<Item = &str> + '_ {
        self.except_once_props.iter()
    }

    /// Returns `true` when the request purpose is `prefetch`.
    pub fn is_prefetch(&self) -> bool {
        self.is_prefetch
    }

    /// Returns `true` when the request cache control contains `no-cache`.
    pub fn is_reload(&self) -> bool {
        self.is_reload
    }

    /// Returns `true` when partial reload headers target `component`.
    pub fn partial_reload_matches(&self, component: &str) -> bool {
        self.is_inertia && self.partial_component.as_deref() == Some(component)
    }

    /// Returns a copy with partial reload headers ignored.
    ///
    /// Framework integrations can use this for non-GET responses, where the
    /// Inertia protocol does not apply partial reload filtering. Once-prop
    /// exclusions are preserved because they are independent of partial reloads.
    pub fn without_partial_reload(mut self) -> Self {
        self.partial_component = None;
        self.partial_data = HeaderList::default();
        self.partial_except = HeaderList::default();
        self.reset = HeaderList::default();
        self.infinite_scroll_merge_intent = None;
        self
    }

    /// Filters a serialized props object according to Inertia request headers.
    ///
    /// Non-object props are left untouched. Object props always contain an
    /// `errors` object after filtering, matching Inertia's page object shape.
    /// When both partial-data and partial-except headers are present,
    /// partial-except takes precedence, matching the Inertia v3 protocol.
    pub fn filter_props(&self, component: &str, props: &mut Value, metadata: &PageMetadata) {
        let Some(props) = props.as_object_mut() else {
            return;
        };

        ensure_errors_prop(props);

        props.retain(|key, _| {
            key == "errors"
                || SelectionPlan::new(EffectiveRequest::new(self, true), component, metadata)
                    .includes(key, SelectionMode::Standard)
        });
    }
}
