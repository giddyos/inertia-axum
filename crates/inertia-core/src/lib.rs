//! Framework-neutral Inertia.js protocol implementation.

#![forbid(unsafe_code)]

mod app;
pub mod assets;
mod engine;
pub mod form;
mod headers;
mod html;
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

#[cfg(feature = "ssr")]
pub mod ssr;

#[cfg(feature = "ssr")]
pub use ssr::{
    Ssr, SsrBackendKind, SsrFailure, SsrFailureKind, SsrHealth, SsrOverride, SsrStartError,
    StartError,
};

pub use app::{ErrorHandler, InertiaApp, InertiaAppBuilder};
pub use assets::{AssetContext, AssetError, AssetProvider, AssetVersion, ConfigError};
pub use engine::{CoreError, PreparedRequest, VersionCheck};
pub use form::{Errors, FormError, InertiaForm as Form, Validate, Validated};
pub use headers::*;
pub use html::HtmlResponseContext;
pub use page::{Inertia, InertiaPageBuilder, OnceProp, Page, PageMetadata, ScrollProps};
pub use props::{
    InertiaProps, InertiaResult, IntoPageProps, IntoScrollPage, LoadPolicy, MergePolicy,
    OncePolicy, Prop, PropError, PropOptions, ScopedInertiaProps, ScrollPage, ScrollPolicy, always,
    defer, lazy, merge, once, optional, scroll,
};
pub use redirect::{Location, Redirect};
pub use request::{RequestContext, RequestParts};
pub use response::{
    CoreBody, CoreResponse, DynamicPage, PendingPage, PendingResponse, PendingValidation,
};
pub use root::{
    AssetTags, AssetTags as RootAssetTags, HeadMarkup, MountMarkup, RootContext, RootView,
};
pub use share::{Share, ShareContext};
pub use transient::{MemoryTransient, TransientData, TransientRequest, TransientStore};
pub use typed::{Component, InertiaPage, IntoInertiaProps, PageOptions, PropKey, Props};
pub use visit::Visit;

#[cfg(feature = "askama")]
pub use askama;

#[cfg(feature = "askama")]
pub use root::{AskamaRoot, AskamaRootContext};

#[cfg(feature = "cookies")]
pub use transient::CookieTransient;

#[cfg(feature = "tower-sessions")]
pub use transient::TowerSessionTransient;

#[cfg(feature = "typegen")]
pub use inertia_macros::InertiaType;

#[cfg(feature = "macros")]
pub use inertia_macros::{InertiaForm, InertiaPage, InertiaProps};

/// Implementation details referenced by exported declarative and derive macros.
#[doc(hidden)]
pub mod __private {
    pub use crate::html::html_response_context;
    pub use crate::page::PageDraft;
    pub use crate::props::prop::{DynamicPropAdapter, IntoPendingProp, PendingProp};
    pub use serde_json::{Value, to_value};

    #[cfg(feature = "typegen")]
    pub use inertia_typegen as typegen;
}

/// Internal type-generation adapter used only by generated exporter tests.
#[doc(hidden)]
#[cfg(feature = "typegen")]
pub mod __typegen {
    pub use inertia_typegen::*;
}
