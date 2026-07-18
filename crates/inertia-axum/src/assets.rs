//! Axum asset-service compatibility exports.

/// Filesystem service used by the Vite compatibility mount.
#[cfg(feature = "vite")]
pub type StaticAssetService = tower_http::services::ServeDir;
