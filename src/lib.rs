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
mod typed;
mod visit;

pub use app::{ErrorHandler, InertiaApp, InertiaAppBuilder, RouterInertiaExt};
pub use assets::{
    AssetContext, AssetError, AssetProvider, AssetVersion, ConfigError, StaticAssetService,
};
pub use headers::*;
pub use html::HtmlResponseContext;
pub use layer::{InertiaLayer, InertiaService};
pub use page::{Inertia, InertiaPageBuilder, OnceProp, Page, PageMetadata, ScrollProps};
pub use props::{
    always, defer, lazy, merge, once, optional, scroll, InertiaProps, InertiaResult, IntoPageProps,
    IntoScrollPage, LoadPolicy, MergePolicy, OncePolicy, Prop, PropError, PropOptions,
    ScopedInertiaProps, ScrollPage, ScrollPolicy,
};
pub use redirect::{Location, Redirect};
pub use request::RequestContext;
pub use response::{DynamicPage, PendingPage, PendingResponse, PendingResponseHandle};
pub use root::{AssetTags, MountMarkup, RootContext, RootView};
pub use typed::{Component, InertiaPage, IntoInertiaProps, PageOptions, PropKey, Props};
pub use visit::Visit;

/// Implementation details referenced by exported declarative macros.
#[doc(hidden)]
pub mod __private {
    pub use crate::props::prop::{DynamicPropAdapter, IntoPendingProp};
    pub use axum::response::{IntoResponse, Response};
}

#[cfg(feature = "macros")]
pub use inertia_axum_macros::{InertiaPage, InertiaProps};

/// Advanced protocol-aware application APIs.
pub mod advanced {
    pub use crate::{
        AssetContext, AssetProvider, AssetTags, AssetVersion, ErrorHandler, MountMarkup,
        RequestContext as InertiaRequestContext, RootContext, RootView, Visit,
    };
}

/// Compatibility APIs retained during the 1.0 alpha migration.
pub mod compat {
    pub use crate::axum::{InertiaRequest, SharedProps, VersionLayer};
    pub use crate::{Inertia, InertiaPageBuilder, InertiaProps, ScopedInertiaProps};
}
