//! An Axum adapter for the Inertia.js protocol.
//!
//! This crate provides request parsing, Inertia page construction, partial
//! reload handling, shared props, asset versioning, redirects, and Axum
//! response integration.

#![forbid(unsafe_code)]

pub mod axum;

mod headers;
mod html;
mod page;
mod props;
mod redirect;
mod request;
mod shared;

pub use headers::*;
pub use html::HtmlResponseContext;
pub use page::{Inertia, InertiaPageBuilder, OnceProp, Page, PageMetadata, ScrollProps};
pub use props::{InertiaProps, IntoPageProps, ScopedInertiaProps};
pub use redirect::{Location, Redirect};
pub use request::RequestContext;
