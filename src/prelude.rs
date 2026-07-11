//! Common application-facing Inertia APIs.

pub use crate::{
    always, defer, lazy, merge, once, optional, page, scroll, DynamicPage, InertiaApp,
    InertiaResult, Location, Prop, Redirect, RouterInertiaExt,
};

#[cfg(feature = "macros")]
pub use crate::{InertiaPage, InertiaProps};
