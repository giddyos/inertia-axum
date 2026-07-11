//! Axum integration for `inertia-axum`.

pub(crate) mod error;
mod extract;
pub mod render;
pub(crate) mod response_headers;
mod shared;
mod version;

pub use crate::HtmlResponseContext;
pub use render::{
    InertiaError, InertiaRequest, InertiaVersion, SharedProps, SharedRequest, VersionLayer,
    VersionService,
};

#[cfg(test)]
mod tests;
