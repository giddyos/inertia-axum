//! Optional, feature-gated project tooling for `inertia-axum`.
#![allow(missing_docs)]

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
#[cfg(feature = "init")]
pub mod output;
pub mod package_manager;
pub mod ssr;
#[cfg(feature = "sync")]
pub mod sync;
#[cfg(feature = "templates")]
pub mod templates;
