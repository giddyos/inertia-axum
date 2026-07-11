//! Common application-facing Inertia APIs.

pub use crate::{
    DynamicPage, Form as InertiaForm, InertiaApp, InertiaResult, Location, MemoryTransient, Prop,
    Redirect, RouterInertiaExt, Share, ShareContext, TransientStore, Validated, always, defer,
    lazy, merge, once, optional, page, scroll,
};

#[cfg(feature = "ssr")]
pub use crate::Ssr;

#[cfg(feature = "cookies")]
pub use crate::CookieTransient;

#[cfg(feature = "macros")]
pub use crate::{InertiaForm, InertiaPage, InertiaProps};
