//! Optional, feature-gated project tooling for `inertia-axum`.

#[cfg(feature = "check")]
pub mod check;
#[cfg(feature = "cli")]
pub mod cli;
#[cfg(feature = "dev")]
pub mod dev;
#[cfg(feature = "cli")]
pub mod error;
pub mod framework;
#[cfg(feature = "init")]
pub mod init;
#[cfg(feature = "package-managers")]
pub mod package_manager;
pub mod ssr;
#[cfg(feature = "templates")]
pub mod templates;
