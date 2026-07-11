//! Public props conversion and the private synchronous resolver type.
//!
//! `IntoPageProps` converts a complete props source to a page JSON object;
//! `PropResolver` type-erases one synchronous lazy resolver without async,
//! `Send`, or `Sync` requirements.

use crate::page::PageMetadata;
use crate::request::RequestContext;
use serde::Serialize;
use serde_json::Value;

/// Converts a value into a filtered Inertia props object.
///
/// Most callers use ordinary serializable structs or maps. [`tyalias@crate::InertiaProps`]
/// implements this trait for route-local lazy, optional, and deferred
/// synchronous resolvers.
pub trait IntoPageProps {
    /// Builds the concrete JSON props object, response metadata, and route
    /// prop roots for shared-prop collision handling.
    fn into_page_props(
        self,
        component: &str,
        request: &RequestContext,
        partial_reload_enabled: bool,
        metadata: PageMetadata,
    ) -> Result<(Value, PageMetadata, Vec<String>), serde_json::Error>;
}

pub(crate) trait PropResolver {
    fn resolve(self: Box<Self>) -> Result<Value, serde_json::Error>;
}

impl<F, T> PropResolver for F
where
    F: FnOnce() -> T,
    T: Serialize,
{
    fn resolve(self: Box<Self>) -> Result<Value, serde_json::Error> {
        serde_json::to_value(self())
    }
}
