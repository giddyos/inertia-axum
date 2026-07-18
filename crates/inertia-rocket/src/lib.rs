//! Rocket adapter for the framework-neutral Inertia runtime.

#![forbid(unsafe_code)]

mod assets;
mod boundary;
mod fairing;
pub mod form;
mod guard;
mod response;

pub use fairing::InertiaFairing;
pub use form::{FormError, InertiaForm, Validated};
pub use guard::{Inertia, Visit};
pub use response::{
    DynamicPage, Error, InertiaResponse, Location, PendingPage, Redirect, Response, Result,
};

pub use inertia_core::{
    AssetBody, AssetContext, AssetError, AssetProvider, AssetRequest, AssetResponse, AssetRuntime,
    AssetSource, AssetTags, AssetVersion, Component, ConfigError, CoreBody, CoreError,
    CoreResponse, ErrorHandler, Errors, Form, HeadMarkup, HtmlResponseContext, InertiaApp,
    InertiaAppBuilder, InertiaPage, InertiaPageBuilder, InertiaProps, InertiaResult,
    IntoInertiaProps, IntoPageProps, IntoScrollPage, LoadPolicy, MemoryTransient, MergePolicy,
    MountMarkup, OncePolicy, OnceProp, Page, PageMetadata, PageOptions, PendingResponse, Prop,
    PropError, PropKey, PropOptions, Props, RequestContext, RequestParts, RootAssetTags,
    RootContext, RootView, ScopedInertiaProps, ScrollPage, ScrollPolicy, ScrollProps, Share,
    ShareContext, TransientData, TransientRequest, TransientStore, Validate, VersionCheck, always,
    defer, lazy, merge, once, optional, scroll,
};

#[cfg(feature = "vite")]
pub use inertia_core::DirectoryAssetSource;

pub use inertia_core::{
    ACCEPT, CACHE_CONTROL, PURPOSE, VARY, X_INERTIA, X_INERTIA_ERROR_BAG,
    X_INERTIA_EXCEPT_ONCE_PROPS, X_INERTIA_HEADER, X_INERTIA_INFINITE_SCROLL_MERGE_INTENT,
    X_INERTIA_LOCATION, X_INERTIA_LOCATION_HEADER, X_INERTIA_PARTIAL_COMPONENT,
    X_INERTIA_PARTIAL_DATA, X_INERTIA_PARTIAL_EXCEPT, X_INERTIA_REDIRECT,
    X_INERTIA_REDIRECT_HEADER, X_INERTIA_RESET, X_INERTIA_VERSION, X_INERTIA_VERSION_HEADER,
    X_REQUESTED_WITH,
};

#[cfg(feature = "cookies")]
pub use inertia_core::CookieTransient;
#[cfg(feature = "macros")]
pub use inertia_core::InertiaForm;
#[cfg(feature = "typegen")]
pub use inertia_core::InertiaType;
#[cfg(feature = "askama")]
pub use inertia_core::{AskamaRoot, AskamaRootContext, askama};
#[cfg(feature = "ssr")]
pub use inertia_core::{
    Ssr, SsrBackendKind, SsrFailure, SsrFailureKind, SsrHealth, SsrOverride, SsrStartError,
    StartError,
};

/// Implementation details referenced by exported macros.
#[doc(hidden)]
pub mod __private {
    pub use inertia_core as core;
    pub use inertia_core::__private::*;
}
