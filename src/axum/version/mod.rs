//! Asset-version middleware and request extension values.

mod future;
use super::error::internal_error_response;
use super::response_headers::{conflict_response, header, original_local_uri};
use crate::X_INERTIA;
use axum::http::{Method, Request};
use axum::response::Response;
use future::VersionFuture;
use std::sync::Arc;
use std::task::{Context, Poll};
use tower_layer::Layer;
use tower_service::Service;

type VersionProvider = Arc<dyn Fn() -> Arc<str> + Send + Sync>;

#[derive(Clone)]
enum VersionSource {
    Static(Arc<str>),
    Dynamic(VersionProvider),
}

impl VersionSource {
    fn resolve(&self) -> Arc<str> {
        match self {
            Self::Static(version) => Arc::clone(version),
            Self::Dynamic(provider) => provider(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InertiaVersion(Arc<str>);

impl InertiaVersion {
    /// Creates an asset version value for request extensions.
    pub fn new<V: Into<Arc<str>>>(version: V) -> Self {
        Self(version.into())
    }

    /// Returns the asset version string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn clone_arc(&self) -> Arc<str> {
        Arc::clone(&self.0)
    }
}

#[derive(Clone)]
pub(crate) struct InertiaVersionSource(VersionSource);

impl InertiaVersionSource {
    pub(crate) fn resolve(&self) -> InertiaVersion {
        InertiaVersion::new(self.0.resolve())
    }
}

/// Shared Inertia props resolved for every Axum page response.
///
/// Register this as an Axum extension layer with [`axum::Extension`]. Shared
/// props are shallow-merged into page props; route props win on key collisions.
/// Providers run once per page response and may inspect the extracted
/// [`crate::axum::InertiaRequest`]. Dotted keys, such as `auth.user`, are expanded into
/// nested props.
///
/// Shared props are merged after partial-reload filtering, so they remain
/// present on partial responses even when omitted from `only` or `except`
/// reload options.
///
/// ```rust,no_run
/// use axum::{Extension, Router};
/// use inertia_axum::axum::{SharedProps, VersionLayer};
///
/// let shared_props = SharedProps::new()
///     .value("appName", "My App")
///     .prop_optional("auth.csrfToken", |request| {
///         request.context().is_inertia().then_some("csrf-token")
///     });
///
/// let app: Router<()> = Router::new()
///     .layer(Extension(shared_props))
///     .layer(VersionLayer::new("asset-version-1"));
/// ```
#[derive(Clone)]
pub struct VersionLayer {
    source: VersionSource,
}

impl VersionLayer {
    /// Creates a layer with a static asset `version`.
    pub fn new<V: Into<Arc<str>>>(version: V) -> Self {
        Self {
            source: VersionSource::Static(version.into()),
        }
    }

    /// Creates a layer with a dynamic asset-version provider.
    ///
    /// Keep the provider fast and non-blocking. If the version is loaded from
    /// disk or a manifest, cache it in application state and read the cached
    /// value here.
    pub fn dynamic<F, V>(version_provider: F) -> Self
    where
        F: Fn() -> V + Send + Sync + 'static,
        V: Into<Arc<str>>,
    {
        Self {
            source: VersionSource::Dynamic(Arc::new(move || version_provider().into())),
        }
    }
}

impl<S> Layer<S> for VersionLayer {
    type Service = VersionService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        VersionService {
            inner,
            source: self.source.clone(),
        }
    }
}

/// Service produced by [`VersionLayer`].
#[derive(Clone)]
pub struct VersionService<S> {
    inner: S,
    source: VersionSource,
}

impl<S, B> Service<Request<B>> for VersionService<S>
where
    S: Service<Request<B>, Response = Response>,
{
    type Response = Response;
    type Error = S::Error;
    type Future = VersionFuture<S::Future, S::Error>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut request: Request<B>) -> Self::Future {
        let should_check =
            request.method() == Method::GET && header(request.headers(), X_INERTIA).is_some();

        if should_check {
            let version = self.source.resolve();
            let request_version = header(request.headers(), crate::X_INERTIA_VERSION);

            if request_version != Some(version.as_ref()) {
                let response = conflict_response(original_local_uri(&request))
                    .unwrap_or_else(internal_error_response);

                return VersionFuture::Ready {
                    result: Some(Ok(response)),
                };
            }

            request
                .extensions_mut()
                .insert(InertiaVersion::new(version));
        } else {
            request
                .extensions_mut()
                .insert(InertiaVersionSource(self.source.clone()));
        }

        VersionFuture::Inner {
            future: self.inner.call(request),
        }
    }
}
