//! Eager and synchronously resolved route props.

mod eager;
mod lazy;
pub(crate) mod prop;
mod resolver;

pub use lazy::{InertiaProps, ScopedInertiaProps};
pub use prop::{
    InertiaResult, IntoScrollPage, LoadPolicy, MergePolicy, OncePolicy, Prop, PropError,
    PropOptions, ScrollPage, ScrollPolicy, always, defer, lazy, merge, once, optional, scroll,
};
pub(crate) use prop::{PendingProp, PendingResolution};
pub use resolver::IntoPageProps;
