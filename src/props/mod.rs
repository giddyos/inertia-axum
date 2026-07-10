//! Eager and lazy page props.

mod eager;
mod lazy;
mod resolver;

pub use crate::page::builder::{InertiaProps, IntoPageProps, ScopedInertiaProps};
