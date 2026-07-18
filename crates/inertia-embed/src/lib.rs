//! Compile-time embedded frontend assets for Inertia applications.

#![forbid(unsafe_code)]

mod cache;
mod frontend;
mod request;

pub use frontend::{EmbeddedAsset, EmbeddedFrontend, EmbeddedStorage};
pub use inertia_embed_macros::embed_frontend;
