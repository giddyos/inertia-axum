//! An Axum adapter for the Inertia.js protocol.
//!
//! This crate provides request parsing, Inertia page construction, partial
//! reload handling, shared props, asset versioning, redirects, and Axum
//! response integration.

#![forbid(unsafe_code)]

pub mod axum;
pub mod prelude;
#[cfg(feature = "ssr")]
pub mod ssr;

mod app;
pub mod assets;
mod engine;
pub mod form;
mod headers;
mod html;
mod layer;
mod page;
mod props;
mod redirect;
mod request;
mod response;
mod root;
mod share;
mod shared;
pub mod transient;
mod typed;
mod visit;

pub use app::{ErrorHandler, InertiaApp, InertiaAppBuilder, RouterInertiaExt};
#[cfg(feature = "vite")]
pub use assets::StaticAssetService;
pub use assets::{AssetContext, AssetError, AssetProvider, AssetVersion, ConfigError};
pub use form::{Errors, FormError, InertiaForm as Form, Validate, Validated};
pub use headers::*;
pub use html::HtmlResponseContext;
pub use layer::{InertiaLayer, InertiaService};
pub use page::{Inertia, InertiaPageBuilder, OnceProp, Page, PageMetadata, ScrollProps};
pub use props::{
    InertiaProps, InertiaResult, IntoPageProps, IntoScrollPage, LoadPolicy, MergePolicy,
    OncePolicy, Prop, PropError, PropOptions, ScopedInertiaProps, ScrollPage, ScrollPolicy, always,
    defer, lazy, merge, once, optional, scroll,
};
pub use redirect::{Location, Redirect};
pub use request::RequestContext;
pub use response::{DynamicPage, PendingPage, PendingResponse, PendingResponseHandle};
pub use root::{AssetTags, HeadMarkup, MountMarkup, RootContext, RootView};
pub use share::{Share, ShareContext};
#[cfg(feature = "ssr")]
pub use ssr::{
    Ssr, SsrBackendKind, SsrContext, SsrFailure, SsrFailureKind, SsrHealth, SsrOverride,
    SsrRouteExt, SsrStartError, StartError,
};
#[cfg(feature = "cookies")]
pub use transient::CookieTransient;
#[cfg(feature = "tower-sessions")]
pub use transient::TowerSessionTransient;
pub use transient::{MemoryTransient, TransientData, TransientRequest, TransientStore};
pub use typed::{Component, InertiaPage, IntoInertiaProps, PageOptions, PropKey, Props};
pub use visit::Visit;

/// Implementation details referenced by exported declarative macros.
#[doc(hidden)]
pub mod __private {
    pub use crate::props::prop::{DynamicPropAdapter, IntoPendingProp};
    pub use axum::response::{IntoResponse, Response};
    pub use serde_json::{Value, to_value};
}

#[cfg(feature = "macros")]
pub use inertia_axum_macros::{InertiaForm, InertiaPage, InertiaProps};

/// Advanced protocol-aware application APIs.
pub mod advanced {
    pub use crate::{
        AssetContext, AssetProvider, AssetTags, AssetVersion, ErrorHandler, MountMarkup,
        RequestContext as InertiaRequestContext, RootContext, RootView, ShareContext,
        TransientStore, Visit,
    };
}

/// Compatibility APIs retained during the 1.0 alpha migration.
pub mod compat {
    pub use crate::axum::{InertiaRequest, SharedProps, VersionLayer};
    pub use crate::{Inertia, InertiaPageBuilder, InertiaProps, ScopedInertiaProps};
}
