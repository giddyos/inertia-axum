//! Inertia page models and builders.

pub mod builder;
mod draft;
mod metadata;
mod model;

pub(crate) use builder::PageDraft;
pub use builder::{Inertia, InertiaPageBuilder, OnceProp, Page, PageMetadata, ScrollProps};
