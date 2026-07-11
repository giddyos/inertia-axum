//! Common application-facing Inertia APIs.

pub use crate::{
    always, defer, lazy, merge, once, optional, page, scroll, DynamicPage, InertiaApp,
    InertiaResult, Location, MemoryTransient, Prop, Redirect, RouterInertiaExt, Share,
    ShareContext, TransientStore,
};

#[cfg(feature = "cookies")]
pub use crate::CookieTransient;

#[cfg(feature = "macros")]
pub use crate::{InertiaPage, InertiaProps};
