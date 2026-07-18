//! Request-dependent prop inclusion planning.
//!
//! This private module answers whether standard, optional, deferred, always,
//! and once props should be resolved for one request.

use super::RequestContext;
use crate::page::PageMetadata;

#[derive(Clone, Copy)]
pub(crate) struct EffectiveRequest<'request> {
    context: &'request RequestContext,
    partial_reload_enabled: bool,
}

impl<'request> EffectiveRequest<'request> {
    pub(crate) fn new(context: &'request RequestContext, partial_reload_enabled: bool) -> Self {
        Self {
            context,
            partial_reload_enabled,
        }
    }

    pub(crate) fn context(self) -> &'request RequestContext {
        self.context
    }

    pub(crate) fn partial_reload_matches(self, component: &str) -> bool {
        self.partial_reload_enabled && self.context.partial_reload_matches(component)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SelectionMode {
    Standard,
    Optional,
}

pub(crate) struct SelectionPlan<'request, 'metadata> {
    request: EffectiveRequest<'request>,
    metadata: &'metadata PageMetadata,
    partial_matches: bool,
}

impl<'request, 'metadata> SelectionPlan<'request, 'metadata> {
    pub(crate) fn new(
        request: EffectiveRequest<'request>,
        component: &str,
        metadata: &'metadata PageMetadata,
    ) -> Self {
        Self {
            partial_matches: request.partial_reload_matches(component),
            request,
            metadata,
        }
    }

    pub(crate) fn includes(&self, prop: &str, mode: SelectionMode) -> bool {
        if prop == "errors" {
            return true;
        }

        let context = self.request.context();
        let explicitly_requested = self.partial_matches && context.partial_data_contains(prop);

        if self
            .metadata
            .deferred_props()
            .values()
            .flatten()
            .any(|candidate| candidate == prop)
            && !explicitly_requested
        {
            return false;
        }

        if self
            .metadata
            .once_props()
            .iter()
            .any(|(key, once)| once.prop() == prop && context.once_prop_is_excluded(key))
            && !explicitly_requested
        {
            return false;
        }

        if self
            .metadata
            .always_props()
            .iter()
            .any(|candidate| candidate == prop)
        {
            return true;
        }

        let included = if !self.partial_matches {
            true
        } else if !context.partial_except_is_empty() {
            !context.partial_except_contains(prop)
        } else if !context.partial_data_is_empty() {
            context.partial_data_contains(prop)
        } else {
            true
        };

        matches!(mode, SelectionMode::Standard) && included
            || matches!(mode, SelectionMode::Optional) && explicitly_requested && included
    }
}
