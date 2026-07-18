//! Owned, composable asynchronous props for direct page responses.

use crate::{OnceProp, PageMetadata, ScrollProps};
use serde::Serialize;
use serde_json::Value;
use std::{
    borrow::Cow,
    error::Error,
    fmt,
    future::Future,
    pin::Pin,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// Error returned by an asynchronous prop resolver.
#[derive(Debug)]
pub struct PropError(Box<dyn Error + Send + Sync>);

impl PropError {
    /// Wraps a resolver error.
    pub fn new(error: impl Error + Send + Sync + 'static) -> Self {
        Self(Box::new(error))
    }
    pub(crate) fn serialization(error: serde_json::Error) -> Self {
        Self::new(error)
    }
}

impl fmt::Display for PropError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl Error for PropError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.0.as_ref())
    }
}

/// Standard result returned by direct prop resolvers.
pub type InertiaResult<T> = Result<T, PropError>;
type BoxResolverFuture<T> = Pin<Box<dyn Future<Output = InertiaResult<T>> + Send + 'static>>;

enum Resolver<T> {
    Immediate(T),
    Async(Box<dyn FnOnce() -> BoxResolverFuture<T> + Send + 'static>),
}

/// Determines when a prop is eligible for resolution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadPolicy {
    /// Ordinary eager field.
    Standard,
    /// Owned lazy resolver selected on normal visits.
    Lazy,
    /// Included even when a partial reload does not request it.
    Always,
    /// Included only when explicitly requested.
    Optional,
    /// Deferred into the named follow-up group.
    Deferred {
        /// Follow-up request group advertised to the client.
        group: Cow<'static, str>,
    },
}

/// Client-side once-prop caching policy.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OncePolicy {
    key: Option<Cow<'static, str>>,
    expires_at: Option<SystemTime>,
    fresh: bool,
}

/// Merge behavior attached to a prop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MergePolicy {
    /// Append at an optional nested path.
    Append {
        /// Optional nested merge path.
        path: Option<Cow<'static, str>>,
        /// Optional identity field for matching existing values.
        match_on: Option<Cow<'static, str>>,
    },
    /// Prepend at an optional nested path.
    Prepend {
        /// Optional nested merge path.
        path: Option<Cow<'static, str>>,
        /// Optional identity field for matching existing values.
        match_on: Option<Cow<'static, str>>,
    },
    /// Deep merge with one or more matching paths.
    Deep {
        /// Identity paths used while recursively merging values.
        match_on: Vec<Cow<'static, str>>,
    },
    /// Infinite-scroll merge and pagination metadata.
    Scroll(ScrollPolicy),
}

/// Infinite-scroll protocol policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScrollPolicy {
    page_name: Cow<'static, str>,
    current: u64,
    previous: Option<u64>,
    next: Option<u64>,
    match_on: Option<Cow<'static, str>>,
}

/// Composable behavior for one direct-response prop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropOptions {
    /// Loading policy.
    pub load: LoadPolicy,
    /// Optional once policy.
    pub once: Option<OncePolicy>,
    /// Optional merge policy.
    pub merge: Option<MergePolicy>,
    /// Whether resolver failures are rescued.
    pub rescue: bool,
}

impl Default for PropOptions {
    fn default() -> Self {
        Self {
            load: LoadPolicy::Standard,
            once: None,
            merge: None,
            rescue: false,
        }
    }
}

/// One immediate or asynchronous value plus composable protocol policies.
pub struct Prop<T> {
    resolver: Resolver<T>,
    options: PropOptions,
}

impl<T> Prop<T> {
    /// Wraps an already available value.
    pub fn immediate(value: T) -> Self {
        Self {
            resolver: Resolver::Immediate(value),
            options: PropOptions::default(),
        }
    }

    fn asynchronous<F, Fut, E>(resolver: F, load: LoadPolicy) -> Self
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<T, E>> + Send + 'static,
        E: Error + Send + Sync + 'static,
    {
        Self {
            resolver: Resolver::Async(Box::new(move || {
                Box::pin(async move { resolver().await.map_err(PropError::new) })
            })),
            options: PropOptions {
                load,
                ..PropOptions::default()
            },
        }
    }

    /// Changes the deferred group.
    pub fn group(mut self, group: impl Into<Cow<'static, str>>) -> Self {
        self.options.load = LoadPolicy::Deferred {
            group: group.into(),
        };
        self
    }
    /// Rescues resolver failures into `rescuedProps`.
    pub fn rescue(mut self) -> Self {
        self.options.rescue = true;
        self
    }
    /// Composes once semantics with this prop.
    pub fn once(mut self) -> Self {
        self.options.once.get_or_insert_with(OncePolicy::default);
        self
    }
    /// Sets the once cache key.
    pub fn key(mut self, key: impl Into<Cow<'static, str>>) -> Self {
        self.options
            .once
            .get_or_insert_with(OncePolicy::default)
            .key = Some(key.into());
        self
    }
    /// Sets an absolute once expiration.
    pub fn expires_at(mut self, at: SystemTime) -> Self {
        self.options
            .once
            .get_or_insert_with(OncePolicy::default)
            .expires_at = Some(at);
        self
    }
    /// Sets a once expiration relative to now.
    pub fn expires_in(self, duration: Duration) -> Self {
        self.expires_at(SystemTime::now() + duration)
    }
    /// Forces the client-cached once value to refresh when `fresh` is true.
    pub fn fresh_if(mut self, fresh: bool) -> Self {
        self.options
            .once
            .get_or_insert_with(OncePolicy::default)
            .fresh = fresh;
        self
    }
    /// Selects append merging at the prop root.
    pub fn append(mut self) -> Self {
        self.options.merge = Some(MergePolicy::Append {
            path: None,
            match_on: None,
        });
        self
    }
    /// Selects append merging at a nested path.
    pub fn append_at(mut self, path: impl Into<Cow<'static, str>>) -> Self {
        self.options.merge = Some(MergePolicy::Append {
            path: Some(path.into()),
            match_on: None,
        });
        self
    }
    /// Selects prepend merging at the prop root.
    pub fn prepend(mut self) -> Self {
        self.options.merge = Some(MergePolicy::Prepend {
            path: None,
            match_on: None,
        });
        self
    }
    /// Selects prepend merging at a nested path.
    pub fn prepend_at(mut self, path: impl Into<Cow<'static, str>>) -> Self {
        self.options.merge = Some(MergePolicy::Prepend {
            path: Some(path.into()),
            match_on: None,
        });
        self
    }
    /// Selects deep merging.
    pub fn deep(mut self) -> Self {
        self.options.merge = Some(MergePolicy::Deep {
            match_on: Vec::new(),
        });
        self
    }
    /// Adds a merge match path.
    pub fn match_on(mut self, path: impl Into<Cow<'static, str>>) -> Self {
        let path = path.into();
        match &mut self.options.merge {
            Some(MergePolicy::Append { match_on, .. } | MergePolicy::Prepend { match_on, .. }) => {
                *match_on = Some(path)
            }
            Some(MergePolicy::Deep { match_on }) => match_on.push(path),
            Some(MergePolicy::Scroll(policy)) => policy.match_on = Some(path),
            None => {
                self.options.merge = Some(MergePolicy::Append {
                    path: None,
                    match_on: Some(path),
                })
            }
        }
        self
    }
    /// Adds a match key relative to a nested merge path.
    pub fn match_on_at(self, path: impl Into<Cow<'static, str>>, key: impl AsRef<str>) -> Self {
        self.match_on(format!("{}.{}", path.into(), key.as_ref()))
    }
    /// Returns the configured policies.
    pub fn options(&self) -> &PropOptions {
        &self.options
    }
}

/// Creates an owned lazy asynchronous prop.
pub fn lazy<T, F, Fut, E>(resolver: F) -> Prop<T>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = Result<T, E>> + Send + 'static,
    E: Error + Send + Sync + 'static,
{
    Prop::asynchronous(resolver, LoadPolicy::Lazy)
}
/// Creates an asynchronous prop included on every matching partial reload.
pub fn always<T: Send + 'static>(value: T) -> Prop<T> {
    let mut prop = Prop::immediate(value);
    prop.options.load = LoadPolicy::Always;
    prop
}
/// Creates an owned asynchronous prop loaded only when explicitly requested.
pub fn optional<T, F, Fut, E>(resolver: F) -> Prop<T>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = Result<T, E>> + Send + 'static,
    E: Error + Send + Sync + 'static,
{
    Prop::asynchronous(resolver, LoadPolicy::Optional)
}
/// Creates an owned deferred asynchronous prop in the default group.
pub fn defer<T, F, Fut, E>(resolver: F) -> Prop<T>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = Result<T, E>> + Send + 'static,
    E: Error + Send + Sync + 'static,
{
    Prop::asynchronous(
        resolver,
        LoadPolicy::Deferred {
            group: Cow::Borrowed("default"),
        },
    )
}
/// Creates an owned asynchronous once prop.
pub fn once<T, F, Fut, E>(resolver: F) -> Prop<T>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = Result<T, E>> + Send + 'static,
    E: Error + Send + Sync + 'static,
{
    lazy(resolver).once()
}
/// Creates an immediate prop ready for merge policies.
pub fn merge<T: Send + 'static>(value: T) -> Prop<T> {
    Prop::immediate(value)
}

/// A normalized page used by the infinite-scroll helper.
#[derive(Clone, Debug, Serialize)]
pub struct ScrollPage<T> {
    data: Vec<T>,
    #[serde(skip)]
    policy: ScrollPolicy,
}

impl<T> ScrollPage<T> {
    /// Creates a normalized scroll page.
    pub fn new(data: Vec<T>, current: u64) -> Self {
        Self {
            data,
            policy: ScrollPolicy {
                page_name: Cow::Borrowed("page"),
                current,
                previous: None,
                next: None,
                match_on: None,
            },
        }
    }
    /// Sets the previous cursor.
    pub fn previous(mut self, value: u64) -> Self {
        self.policy.previous = Some(value);
        self
    }
    /// Sets the next cursor.
    pub fn next(mut self, value: u64) -> Self {
        self.policy.next = Some(value);
        self
    }
    /// Sets the page query parameter.
    pub fn page_name(mut self, value: impl Into<Cow<'static, str>>) -> Self {
        self.policy.page_name = value.into();
        self
    }
}

/// Converts application pagination into a normalized scroll page.
pub trait IntoScrollPage {
    /// Item serialized in the normalized page data array.
    type Item: Serialize + Send + 'static;
    /// Converts application pagination to the normalized wire model.
    fn into_scroll_page(self) -> ScrollPage<Self::Item>;
}
impl<T: Serialize + Send + 'static> IntoScrollPage for ScrollPage<T> {
    type Item = T;
    fn into_scroll_page(self) -> Self {
        self
    }
}

/// Creates an immediate infinite-scroll prop.
pub fn scroll<P: IntoScrollPage>(page: P) -> Prop<ScrollPage<P::Item>> {
    let page = page.into_scroll_page();
    let policy = page.policy.clone();
    let mut prop = Prop::immediate(page);
    prop.options.merge = Some(MergePolicy::Scroll(policy));
    prop
}

type ErasedValueFuture = Pin<Box<dyn Future<Output = InertiaResult<Value>> + Send + 'static>>;
type ResolvedPropFuture =
    Pin<Box<dyn Future<Output = (String, InertiaResult<Value>, bool)> + Send + 'static>>;

pub(crate) enum ErasedResolver {
    Sync(Box<dyn FnOnce() -> InertiaResult<Value> + Send>),
    Async(Box<dyn FnOnce() -> ErasedValueFuture + Send>),
}

#[doc(hidden)]
pub struct PendingProp {
    pub(crate) key: String,
    pub(crate) options: PropOptions,
    resolver: ErasedResolver,
}

impl PendingProp {
    pub(crate) fn apply_metadata(&self, metadata: &mut PageMetadata, include_once: bool) {
        let key = &self.key;
        match &self.options.load {
            LoadPolicy::Always => metadata.add_always(key.clone()),
            LoadPolicy::Deferred { group } => metadata.add_deferred(group.to_string(), key.clone()),
            LoadPolicy::Standard | LoadPolicy::Lazy | LoadPolicy::Optional => {}
        }
        if include_once {
            if let Some(once) = &self.options.once {
                let once_key = once.key.as_deref().unwrap_or(key).to_owned();
                let mut value = OnceProp::new(key.clone());
                if let Some(at) = once
                    .expires_at
                    .and_then(|at| at.duration_since(UNIX_EPOCH).ok())
                {
                    value = value.expires_at(u64::try_from(at.as_millis()).unwrap_or(u64::MAX));
                }
                metadata.add_once(once_key, value);
            }
        }
        match &self.options.merge {
            Some(MergePolicy::Append { path, match_on }) => {
                let target = qualify(key, path.as_deref());
                *metadata = std::mem::take(metadata).merge(target);
                if let Some(path) = match_on {
                    *metadata = std::mem::take(metadata).match_on(qualify(key, Some(path)));
                }
            }
            Some(MergePolicy::Prepend { path, match_on }) => {
                let target = qualify(key, path.as_deref());
                *metadata = std::mem::take(metadata).prepend(target);
                if let Some(path) = match_on {
                    *metadata = std::mem::take(metadata).match_on(qualify(key, Some(path)));
                }
            }
            Some(MergePolicy::Deep { match_on }) => {
                *metadata = std::mem::take(metadata).deep_merge(key.clone());
                for path in match_on {
                    *metadata = std::mem::take(metadata).match_on(qualify(key, Some(path)));
                }
            }
            Some(MergePolicy::Scroll(policy)) => {
                let mut scroll = ScrollProps::new(policy.page_name.to_string(), policy.current);
                if let Some(previous) = policy.previous {
                    scroll = scroll.previous_page(previous);
                }
                if let Some(next) = policy.next {
                    scroll = scroll.next_page(next);
                }
                *metadata = std::mem::take(metadata).scroll(key.clone(), scroll);
                if let Some(path) = &policy.match_on {
                    *metadata = std::mem::take(metadata).match_on(qualify(key, Some(path)));
                }
            }
            None => {}
        }
    }

    pub(crate) fn apply_shared_metadata(&self, metadata: &mut PageMetadata) {
        self.apply_metadata(metadata, !self.is_fresh_once());
        if matches!(self.options.load, LoadPolicy::Standard | LoadPolicy::Lazy) {
            metadata.add_always(self.key.clone());
        }
    }

    pub(crate) fn mode(&self) -> crate::request::SelectionMode {
        if matches!(self.options.load, LoadPolicy::Optional) {
            crate::request::SelectionMode::Optional
        } else {
            crate::request::SelectionMode::Standard
        }
    }
    pub(crate) fn is_fresh_once(&self) -> bool {
        self.options.once.as_ref().is_some_and(|once| once.fresh)
    }
    pub(crate) fn into_resolution(self) -> PendingResolution {
        let rescue = self.options.rescue;
        let key = self.key;
        match self.resolver {
            ErasedResolver::Sync(resolve) => PendingResolution::Ready((key, resolve(), rescue)),
            ErasedResolver::Async(resolve) => {
                PendingResolution::Async(Box::pin(async move { (key, resolve().await, rescue) }))
            }
        }
    }
}

pub(crate) enum PendingResolution {
    Ready((String, InertiaResult<Value>, bool)),
    Async(ResolvedPropFuture),
}

fn qualify(key: &str, path: Option<&str>) -> String {
    path.map_or_else(|| key.to_owned(), |path| format!("{key}.{path}"))
}

#[doc(hidden)]
pub struct DynamicPropAdapter<T>(Option<T>);
impl<T> DynamicPropAdapter<T> {
    pub fn new(value: T) -> Self {
        Self(Some(value))
    }
}

#[doc(hidden)]
pub trait IntoPendingProp {
    fn into_pending_prop(self, key: String) -> PendingProp;
}

impl<T> IntoPendingProp for DynamicPropAdapter<Prop<T>>
where
    T: Serialize + Send + 'static,
{
    fn into_pending_prop(mut self, key: String) -> PendingProp {
        let prop = self.0.take().expect("prop adapter consumed once");
        let resolver = match prop.resolver {
            Resolver::Immediate(value) => ErasedResolver::Sync(Box::new(move || {
                serde_json::to_value(value).map_err(PropError::serialization)
            })),
            Resolver::Async(resolve) => ErasedResolver::Async(Box::new(move || {
                Box::pin(async move {
                    serde_json::to_value(resolve().await?).map_err(PropError::serialization)
                })
            })),
        };
        PendingProp {
            key,
            options: prop.options,
            resolver,
        }
    }
}

impl<T> IntoPendingProp for &mut DynamicPropAdapter<T>
where
    T: Serialize + Send + 'static,
{
    fn into_pending_prop(self, key: String) -> PendingProp {
        let value = self.0.take().expect("prop adapter consumed once");
        PendingProp {
            key,
            options: PropOptions::default(),
            resolver: ErasedResolver::Sync(Box::new(move || {
                serde_json::to_value(value).map_err(PropError::serialization)
            })),
        }
    }
}
