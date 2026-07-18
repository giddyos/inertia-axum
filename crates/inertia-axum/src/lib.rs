//! Axum adapter for the framework-neutral Inertia.js Rust runtime.

#![forbid(unsafe_code)]

pub mod assets;
pub mod axum;
mod extract;
pub mod form;
mod layer;
pub mod prelude;
mod response;
mod router;

#[cfg(feature = "ssr")]
mod ssr;

pub use extract::{Inertia, Visit};
pub use form::{FormError, InertiaForm as Form, Validated};
pub use layer::{InertiaLayer, InertiaService};
pub use response::{
    AxumResponse, DynamicPage, Location, PendingPage, PendingResponseHandle, Redirect,
};
pub use router::RouterInertiaExt;

pub use inertia_core::{
    ACCEPT, CACHE_CONTROL, PURPOSE, VARY, X_INERTIA, X_INERTIA_ERROR_BAG,
    X_INERTIA_EXCEPT_ONCE_PROPS, X_INERTIA_HEADER, X_INERTIA_INFINITE_SCROLL_MERGE_INTENT,
    X_INERTIA_LOCATION, X_INERTIA_LOCATION_HEADER, X_INERTIA_PARTIAL_COMPONENT,
    X_INERTIA_PARTIAL_DATA, X_INERTIA_PARTIAL_EXCEPT, X_INERTIA_REDIRECT,
    X_INERTIA_REDIRECT_HEADER, X_INERTIA_RESET, X_INERTIA_VERSION, X_INERTIA_VERSION_HEADER,
    X_REQUESTED_WITH,
};
pub use inertia_core::{
    AssetContext, AssetError, AssetProvider, AssetVersion, Component, ConfigError, CoreBody,
    CoreError, CoreResponse, ErrorHandler, Errors, HeadMarkup, HtmlResponseContext, InertiaApp,
    InertiaAppBuilder, InertiaPage, InertiaPageBuilder, InertiaProps, InertiaResult,
    IntoInertiaProps, IntoPageProps, IntoScrollPage, LoadPolicy, MemoryTransient, MergePolicy,
    MountMarkup, OncePolicy, OnceProp, Page, PageMetadata, PageOptions, Prop, PropError, PropKey,
    PropOptions, Props, RequestContext, RootAssetTags as AssetTags, RootContext, RootView,
    ScopedInertiaProps, ScrollPage, ScrollPolicy, ScrollProps, Share, ShareContext, TransientData,
    TransientRequest, TransientStore, Validate, VersionCheck, always, defer, lazy, merge, once,
    optional, scroll,
};

#[cfg(feature = "askama")]
pub use inertia_core::{AskamaRoot, AskamaRootContext, askama};

#[cfg(feature = "cookies")]
pub use inertia_core::CookieTransient;

#[cfg(feature = "tower-sessions")]
pub use inertia_core::TowerSessionTransient;

#[cfg(feature = "typegen")]
pub use inertia_core::InertiaType;

#[cfg(feature = "macros")]
pub use inertia_core::InertiaForm;

#[cfg(feature = "vite")]
pub use assets::StaticAssetService;

#[cfg(feature = "ssr")]
pub use inertia_core::ssr::{
    Ssr, SsrBackendKind, SsrFailure, SsrFailureKind, SsrHealth, SsrStartError, StartError,
};

#[cfg(feature = "ssr")]
pub use ssr::{SsrContext, SsrOverride, SsrRouteExt};

/// Implementation details referenced by framework-neutral derives.
#[doc(hidden)]
pub mod __private {
    pub use inertia_core as core;
    pub use inertia_core::__private::*;
}

/// Internal type-generation adapter used by generated exporter tests.
#[doc(hidden)]
#[cfg(feature = "typegen")]
pub mod __typegen {
    pub use inertia_core::__typegen::*;
}

/// Advanced protocol-aware application APIs.
pub mod advanced {
    #[cfg(feature = "askama")]
    pub use crate::{AskamaRoot, AskamaRootContext};
    pub use crate::{
        AssetContext, AssetProvider, AssetTags, AssetVersion, ErrorHandler, MountMarkup,
        RootContext, RootView, ShareContext, TransientStore, Visit,
    };
}

/// Compatibility APIs retained during the 1.0 alpha migration.
pub mod compat {
    pub use crate::axum::{InertiaRequest, SharedProps, VersionLayer};
    pub use crate::{Inertia, InertiaPageBuilder, InertiaProps, ScopedInertiaProps};
    pub use inertia_core::Inertia as LegacyInertia;
}
