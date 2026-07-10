//! Axum integration for `inertia-axum`.

mod error;
mod extract;
pub mod render;
mod response_headers;
mod shared;
mod version;

pub use crate::HtmlResponseContext;
pub use render::{
    InertiaError, InertiaRequest, InertiaVersion, SharedProps, SharedRequest, VersionLayer,
    VersionService,
};
