//! Immutable application configuration and Axum router installation.

use crate::{
    layer::InertiaLayer,
    root::{DefaultRoot, RootView, SharedRootView},
};
use axum::Router;
use std::sync::Arc;

/// Immutable application-wide Inertia configuration.
#[derive(Clone)]
pub struct InertiaApp {
    pub(crate) inner: Arc<InertiaAppInner>,
}

pub(crate) struct InertiaAppInner {
    pub(crate) root: SharedRootView,
    pub(crate) version: Option<Arc<str>>,
}

/// Builds an [`InertiaApp`].
pub struct InertiaAppBuilder {
    root: SharedRootView,
    version: Option<Arc<str>>,
}

impl InertiaApp {
    /// Starts an application setup with a custom root renderer.
    pub fn builder(root: impl RootView) -> InertiaAppBuilder {
        InertiaAppBuilder {
            root: Arc::new(root),
            version: None,
        }
    }

    /// Starts a setup with the safe built-in root renderer.
    pub fn default_root() -> InertiaAppBuilder {
        InertiaAppBuilder {
            root: Arc::new(DefaultRoot),
            version: None,
        }
    }
}

impl Default for InertiaAppBuilder {
    fn default() -> Self {
        InertiaApp::default_root()
    }
}

impl InertiaAppBuilder {
    /// Replaces the application root renderer.
    pub fn root(mut self, root: impl RootView) -> Self {
        self.root = Arc::new(root);
        self
    }

    /// Sets the string asset version used for pre-handler version checks.
    pub fn version(mut self, version: impl Into<Arc<str>>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Builds the immutable application object.
    pub fn build(self) -> Result<InertiaApp, std::convert::Infallible> {
        Ok(InertiaApp {
            inner: Arc::new(InertiaAppInner {
                root: self.root,
                version: self.version,
            }),
        })
    }
}

/// Installs an [`InertiaApp`] on an Axum router.
pub trait RouterInertiaExt<S> {
    /// Adds request parsing, version checks, and pending-response finalization.
    fn inertia(self, app: InertiaApp) -> Self;
}

impl<S> RouterInertiaExt<S> for Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn inertia(self, app: InertiaApp) -> Self {
        self.layer(InertiaLayer::new(app))
    }
}
