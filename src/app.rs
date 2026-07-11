//! Immutable application configuration and Axum router installation.

use crate::{
    assets::{AssetProvider, AssetRuntime, ConfigError, ErasedAssetProvider, ViteConfig},
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
    pub(crate) assets: AssetRuntime,
    pub(crate) error_handler: Option<Arc<dyn ErasedErrorHandler>>,
}

/// Builds an [`InertiaApp`].
pub struct InertiaAppBuilder {
    root: SharedRootView,
    assets: AssetRuntime,
    vite: Option<ViteConfig>,
    asset_provider: Option<Arc<dyn ErasedAssetProvider>>,
    public_path: String,
    error_handler: Option<Arc<dyn ErasedErrorHandler>>,
}

/// Receives deterministic reports for rescued asynchronous prop failures.
pub trait ErrorHandler: Clone + Send + Sync + 'static {
    /// Reports a resolver failure after its prop has been identified.
    fn handle(&self, prop: &str, error: &crate::PropError);
}

pub(crate) trait ErasedErrorHandler: Send + Sync {
    fn handle(&self, prop: &str, error: &crate::PropError);
}

impl<T: ErrorHandler> ErasedErrorHandler for T {
    fn handle(&self, prop: &str, error: &crate::PropError) {
        ErrorHandler::handle(self, prop, error);
    }
}

impl InertiaApp {
    /// Starts an application setup with a custom root renderer.
    pub fn builder(root: impl RootView) -> InertiaAppBuilder {
        InertiaAppBuilder {
            root: Arc::new(root),
            assets: AssetRuntime::default(),
            vite: None,
            asset_provider: None,
            public_path: "/build".to_owned(),
            error_handler: None,
        }
    }

    /// Starts a setup with the safe built-in root renderer.
    pub fn default_root() -> InertiaAppBuilder {
        InertiaAppBuilder {
            root: Arc::new(DefaultRoot),
            assets: AssetRuntime::default(),
            vite: None,
            asset_provider: None,
            public_path: "/build".to_owned(),
            error_handler: None,
        }
    }

    /// Starts a convention-based Vite setup rooted at `root`.
    pub fn vite(root: impl Into<std::path::PathBuf>) -> InertiaAppBuilder {
        InertiaAppBuilder {
            root: Arc::new(DefaultRoot),
            assets: AssetRuntime::default(),
            vite: Some(ViteConfig {
                root: root.into(),
                entry: "src/main.ts".into(),
                build_dir: "dist".into(),
                public_path: "/build".to_owned(),
                dev_server: None,
            }),
            asset_provider: None,
            public_path: "/build".to_owned(),
            error_handler: None,
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
        let version = version.into();
        self.assets.version = Some(version.clone().into());
        self.assets.header_version = Some(version);
        self
    }

    /// Overrides the Vite entry path relative to its root.
    pub fn entry(mut self, entry: impl Into<std::path::PathBuf>) -> Self {
        if let Some(vite) = &mut self.vite {
            vite.entry = entry.into();
        }
        self
    }

    /// Overrides the Vite build directory relative to its root.
    pub fn build_dir(mut self, path: impl Into<std::path::PathBuf>) -> Self {
        if let Some(vite) = &mut self.vite {
            vite.build_dir = path.into();
        }
        self
    }

    /// Overrides the URL prefix used to serve production assets.
    pub fn public_path(mut self, path: impl Into<String>) -> Self {
        let path = path.into();
        self.public_path.clone_from(&path);
        if let Some(vite) = &mut self.vite {
            vite.public_path = path;
        }
        self
    }

    /// Uses an application-defined asset provider instead of Vite.
    pub fn assets<P: AssetProvider>(mut self, provider: P) -> Self {
        self.vite = None;
        self.asset_provider = Some(Arc::new(provider));
        self
    }

    /// Installs the application-wide rescued-prop error reporter.
    pub fn error_handler<E: ErrorHandler>(mut self, handler: E) -> Self {
        self.error_handler = Some(Arc::new(handler));
        self
    }

    /// Overrides the Vite development server URL.
    pub fn dev_server(mut self, url: impl Into<String>) -> Self {
        if let Some(vite) = &mut self.vite {
            vite.dev_server = Some(url.into());
        }
        self
    }

    /// Builds the immutable application object.
    pub fn build(mut self) -> Result<InertiaApp, ConfigError> {
        if let Some(vite) = self.vite {
            self.assets = vite.build()?;
        } else if let Some(provider) = self.asset_provider {
            self.assets = provider.build_runtime(&self.public_path)?;
        }
        Ok(InertiaApp {
            inner: Arc::new(InertiaAppInner {
                root: self.root,
                assets: self.assets,
                error_handler: self.error_handler,
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
        let static_mount = app.inner.assets.static_mount.clone();
        let router = if let Some((path, service)) = static_mount {
            self.nest_service(&path, service)
        } else {
            self
        };
        router.layer(InertiaLayer::new(app))
    }
}
