//! Immutable application configuration and Axum router installation.

#[cfg(feature = "vite")]
use crate::assets::ViteConfig;
use crate::{
    assets::{AssetProvider, AssetRuntime, ConfigError, ErasedAssetProvider},
    layer::InertiaLayer,
    root::{DefaultRoot, RootView, SharedRootView},
    share::{Share, SharedProvider},
    transient::{SharedTransientStore, TransientStore},
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
    pub(crate) shared: Option<SharedProvider>,
    pub(crate) transient: Option<SharedTransientStore>,
    #[cfg(feature = "ssr")]
    pub(crate) ssr: Option<crate::ssr::runtime::SsrRuntime>,
}

/// Builds an [`InertiaApp`].
pub struct InertiaAppBuilder {
    root: SharedRootView,
    assets: AssetRuntime,
    #[cfg(feature = "vite")]
    vite: Option<ViteConfig>,
    asset_provider: Option<Arc<dyn ErasedAssetProvider>>,
    public_path: String,
    error_handler: Option<Arc<dyn ErasedErrorHandler>>,
    shared: Option<SharedProvider>,
    transient: Option<SharedTransientStore>,
    #[cfg(feature = "ssr")]
    ssr: Option<crate::Ssr>,
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
    /// Returns the latest locally recorded SSR health without network I/O.
    #[cfg(feature = "ssr")]
    pub fn ssr_health(&self) -> crate::SsrHealth {
        self.inner
            .ssr
            .as_ref()
            .map(crate::ssr::runtime::SsrRuntime::health)
            .unwrap_or(crate::SsrHealth::Disabled)
    }
    /// Starts an application setup with a custom root renderer.
    pub fn builder(root: impl RootView) -> InertiaAppBuilder {
        InertiaAppBuilder {
            root: Arc::new(root),
            assets: AssetRuntime::default(),
            #[cfg(feature = "vite")]
            vite: None,
            asset_provider: None,
            public_path: "/build".to_owned(),
            error_handler: None,
            shared: None,
            transient: None,
            #[cfg(feature = "ssr")]
            ssr: None,
        }
    }

    /// Starts a setup with the safe built-in root renderer.
    pub fn default_root() -> InertiaAppBuilder {
        InertiaAppBuilder {
            root: Arc::new(DefaultRoot),
            assets: AssetRuntime::default(),
            #[cfg(feature = "vite")]
            vite: None,
            asset_provider: None,
            public_path: "/build".to_owned(),
            error_handler: None,
            shared: None,
            transient: None,
            #[cfg(feature = "ssr")]
            ssr: None,
        }
    }

    /// Starts a convention-based Vite setup rooted at `root`.
    #[cfg(feature = "vite")]
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
            shared: None,
            transient: None,
            #[cfg(feature = "ssr")]
            ssr: None,
        }
    }
}

impl Default for InertiaAppBuilder {
    fn default() -> Self {
        InertiaApp::default_root()
    }
}

impl InertiaAppBuilder {
    fn finish_assets(&mut self) -> Result<(), ConfigError> {
        #[cfg(feature = "vite")]
        if let Some(vite) = self.vite.take() {
            self.assets = vite.build()?;
        } else if let Some(provider) = self.asset_provider.take() {
            self.assets = provider.build_runtime(&self.public_path)?;
        }
        #[cfg(not(feature = "vite"))]
        if let Some(provider) = self.asset_provider.take() {
            self.assets = provider.build_runtime(&self.public_path)?;
        }
        Ok(())
    }

    fn into_app(
        self,
        #[cfg(feature = "ssr")] ssr: Option<crate::ssr::runtime::SsrRuntime>,
    ) -> InertiaApp {
        InertiaApp {
            inner: Arc::new(InertiaAppInner {
                root: self.root,
                assets: self.assets,
                error_handler: self.error_handler,
                shared: self.shared,
                transient: self.transient,
                #[cfg(feature = "ssr")]
                ssr,
            }),
        }
    }

    /// Configures server-side rendering.
    ///
    /// Configuring SSR enables it for all eligible routes by default.
    #[cfg(feature = "ssr")]
    pub fn ssr(mut self, config: impl Into<crate::Ssr>) -> Self {
        self.ssr = Some(config.into());
        self
    }

    /// Replaces the application root renderer.
    pub fn root(mut self, root: impl RootView) -> Self {
        self.root = Arc::new(root);
        self
    }

    /// Sets the string asset version used for pre-handler version checks.
    pub fn version(mut self, version: impl Into<crate::AssetVersion>) -> Self {
        let version = version.into();
        self.assets.header_version = Some(Arc::from(version.header_value().into_owned()));
        self.assets.version = Some(version);
        self
    }

    /// Overrides the Vite entry path relative to its root.
    #[cfg(feature = "vite")]
    pub fn entry(mut self, entry: impl Into<std::path::PathBuf>) -> Self {
        if let Some(vite) = &mut self.vite {
            vite.entry = entry.into();
        }
        self
    }

    /// Overrides the Vite build directory relative to its root.
    #[cfg(feature = "vite")]
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
        #[cfg(feature = "vite")]
        if let Some(vite) = &mut self.vite {
            vite.public_path = path;
        }
        self
    }

    /// Uses an application-defined asset provider instead of Vite.
    pub fn assets<P: AssetProvider>(mut self, provider: P) -> Self {
        #[cfg(feature = "vite")]
        {
            self.vite = None;
        }
        self.asset_provider = Some(Arc::new(provider));
        self
    }

    /// Installs the application-wide rescued-prop error reporter.
    pub fn error_handler<E: ErrorHandler>(mut self, handler: E) -> Self {
        self.error_handler = Some(Arc::new(handler));
        self
    }

    /// Installs the one typed global shared-data provider.
    pub fn share<S: Share>(mut self, provider: S) -> Self {
        self.shared = Some(Arc::new(provider));
        self
    }

    /// Installs request-to-request transient storage.
    pub fn transient<T: TransientStore>(mut self, store: T) -> Self {
        self.transient = Some(Arc::new(store));
        self
    }

    /// Overrides the Vite development server URL.
    #[cfg(feature = "vite")]
    pub fn dev_server(mut self, url: impl Into<String>) -> Self {
        if let Some(vite) = &mut self.vite {
            vite.dev_server = Some(url.into());
        }
        self
    }

    /// Builds the immutable application object.
    pub fn build(mut self) -> Result<InertiaApp, ConfigError> {
        #[cfg(feature = "ssr")]
        if self.ssr.is_some() {
            return Err(ConfigError::new(
                "inertia-axum SSR configuration error\n\n\
                 SSR was configured with InertiaAppBuilder::ssr(...), but build() cannot start an SSR runtime.\n\n\
                 Use:\n\n\
                 let inertia = InertiaApp::vite(\"frontend\")\n\
                     .ssr(\"dist/ssr/ssr.js\")\n\
                     .start()\n\
                     .await?;",
            ));
        }

        self.finish_assets()?;
        Ok(self.into_app(
            #[cfg(feature = "ssr")]
            None,
        ))
    }

    /// Builds assets and starts the configured SSR runtime.
    #[cfg(feature = "ssr")]
    pub async fn start(mut self) -> Result<InertiaApp, crate::StartError> {
        let Some(config) = self.ssr.take() else {
            self.finish_assets()?;
            return Ok(self.into_app(None));
        };
        #[cfg(feature = "vite")]
        let vite_root = self.vite.as_ref().map(|vite| vite.root.clone());
        #[cfg(not(feature = "vite"))]
        let vite_root: Option<std::path::PathBuf> = None;
        self.finish_assets()?;
        let runtime =
            crate::ssr::runtime::start_runtime(config, &self.assets, vite_root.as_deref()).await?;
        Ok(self.into_app(Some(runtime)))
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
        #[cfg(feature = "vite")]
        {
            let static_mount = app.inner.assets.static_mount.clone();
            let router = if let Some((path, service)) = static_mount {
                self.nest_service(&path, service)
            } else {
                self
            };
            router.layer(InertiaLayer::new(app))
        }
        #[cfg(not(feature = "vite"))]
        self.layer(InertiaLayer::new(app))
    }
}
