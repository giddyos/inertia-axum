//! Optional server-side rendering support.

mod config;
mod policy;

pub use config::Ssr;
pub use policy::{SsrContext, SsrOverride, SsrRouteExt};
