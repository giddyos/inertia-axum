//! Inertia page models, metadata, and construction.

mod builder;
mod draft;
mod metadata;
mod model;

pub use builder::{Inertia, InertiaPageBuilder};
pub(crate) use draft::PageDraft;
pub use metadata::{OnceProp, PageMetadata, ScrollProps};
pub use model::Page;

#[cfg(test)]
mod tests;
