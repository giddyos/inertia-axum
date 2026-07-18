//! Axum router installation and filesystem asset mounting.

use crate::InertiaLayer;
use axum::Router;

/// Installs an Inertia application on an Axum router.
pub trait RouterInertiaExt<S> {
    /// Adds request preparation and response finalization.
    fn inertia(self, app: inertia_core::InertiaApp) -> Self;

    /// Alias matching the installation vocabulary used by other adapters.
    fn with_inertia(self, app: inertia_core::InertiaApp) -> Self;
}

impl<S> RouterInertiaExt<S> for Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn inertia(self, app: inertia_core::InertiaApp) -> Self {
        #[cfg(feature = "vite")]
        let router = if let Some((path, root)) = app.__filesystem_mount() {
            self.nest_service(&path, tower_http::services::ServeDir::new(root))
        } else {
            self
        };
        #[cfg(not(feature = "vite"))]
        let router = self;
        router.layer(InertiaLayer::new(app))
    }

    fn with_inertia(self, app: inertia_core::InertiaApp) -> Self {
        self.inertia(app)
    }
}
