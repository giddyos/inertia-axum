//! One typed application-wide shared-data provider.

use crate::{IntoInertiaProps, Props, Visit};
use http::{Extensions, HeaderMap, Method, Uri};
use std::{error::Error, sync::Arc};

/// Borrowed request data available during synchronous shared preparation.
#[derive(Clone, Copy)]
pub struct ShareContext<'a> {
    method: &'a Method,
    uri: &'a Uri,
    headers: &'a HeaderMap,
    extensions: &'a Extensions,
    visit: &'a Visit,
}

impl<'a> ShareContext<'a> {
    pub(crate) fn new(
        method: &'a Method,
        uri: &'a Uri,
        headers: &'a HeaderMap,
        extensions: &'a Extensions,
        visit: &'a Visit,
    ) -> Self {
        Self {
            method,
            uri,
            headers,
            extensions,
            visit,
        }
    }
    /// Returns the request method.
    pub fn method(&self) -> &Method {
        self.method
    }
    /// Returns the request URI.
    pub fn uri(&self) -> &Uri {
        self.uri
    }
    /// Returns the borrowed request headers.
    pub fn headers(&self) -> &HeaderMap {
        self.headers
    }
    /// Returns the parsed Inertia visit.
    pub fn visit(&self) -> &Visit {
        self.visit
    }
    /// Borrows one explicitly requested extension value without cloning the map.
    pub fn extension<T: Send + Sync + 'static>(&self) -> Option<&T> {
        self.extensions.get::<T>()
    }
}

/// Produces one small typed shared-props object per request.
pub trait Share: Clone + Send + Sync + 'static {
    /// Derived shared-props type.
    type Props: IntoInertiaProps;
    /// Synchronous preparation error.
    type Error: Error + Send + Sync + 'static;
    /// Prepares owned shared props while request parts remain borrowable.
    fn share(&self, context: ShareContext<'_>) -> Result<Self::Props, Self::Error>;
}

pub(crate) trait ErasedShare: Send + Sync {
    fn prepare(&self, context: ShareContext<'_>) -> Result<Props, Box<dyn Error + Send + Sync>>;
}

impl<T: Share> ErasedShare for T {
    fn prepare(&self, context: ShareContext<'_>) -> Result<Props, Box<dyn Error + Send + Sync>> {
        self.share(context)
            .map(IntoInertiaProps::into_inertia_props)
            .map_err(|error| Box::new(error) as _)
    }
}

pub(crate) type SharedProvider = Arc<dyn ErasedShare>;
