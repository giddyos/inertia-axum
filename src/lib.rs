//! An Axum adapter for the Inertia.js protocol.
//!
//! This crate provides request parsing, Inertia page construction, partial
//! reload handling, shared props, asset versioning, redirects, and Axum
//! response integration.

#![forbid(unsafe_code)]

pub mod axum;
pub mod prelude;

mod app;
pub mod assets;
mod engine;
mod headers;
mod html;
mod layer;
mod page;
mod props;
mod redirect;
mod request;
mod response;
mod root;
mod shared;
mod visit;

pub use app::{InertiaApp, InertiaAppBuilder, RouterInertiaExt};
pub use assets::{
    AssetContext, AssetError, AssetProvider, AssetVersion, ConfigError, StaticAssetService,
};
pub use headers::*;
pub use html::HtmlResponseContext;
pub use layer::{InertiaLayer, InertiaService};
pub use page::{Inertia, InertiaPageBuilder, OnceProp, Page, PageMetadata, ScrollProps};
pub use props::{InertiaProps, IntoPageProps, ScopedInertiaProps};
pub use redirect::{Location, Redirect};
pub use request::RequestContext;
pub use response::{DynamicPage, PendingPage, PendingResponse, PendingResponseHandle};
pub use root::{AssetTags, MountMarkup, RootContext, RootView};
pub use visit::Visit;

/// Advanced protocol-aware application APIs.
pub mod advanced {
    pub use crate::{
        AssetTags, MountMarkup, RequestContext as InertiaRequestContext, RootContext, RootView,
        Visit,
    };
}

/// Compatibility APIs retained during the 1.0 alpha migration.
pub mod compat {
    pub use crate::axum::{InertiaRequest, SharedProps, VersionLayer};
    pub use crate::{Inertia, InertiaPageBuilder, InertiaProps, ScopedInertiaProps};
}
