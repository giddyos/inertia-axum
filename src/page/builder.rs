use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;

use crate::page::{OnceProp, Page, PageDraft, PageMetadata, ScrollProps};
use crate::props::IntoPageProps;
use crate::redirect::{Location, Redirect};
use crate::request::RequestContext;

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
        self.into_page_draft_version(
            default_url,
            version.map(crate::AssetVersion::from),
            request,
            partial_reload_enabled,
        )
    }

    pub(crate) fn into_page_draft_version(
        self,
        default_url: &str,
        version: Option<crate::AssetVersion>,
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
            Page::from_parts_version(component, props, url, version, metadata),
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
