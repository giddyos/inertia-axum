//! Inertia protocol header constants owned by the framework-neutral crate.
//!
//! The module stays private; [`crate`] re-exports these constants at the root
//! to preserve the public API used by applications and integrations.

use ::axum::http::header::HeaderName;

/// Request header set by Inertia XHR visits.
pub const X_REQUESTED_WITH: &str = "X-Requested-With";

/// Request header containing accepted response content types.
pub const ACCEPT: &str = "Accept";

/// Request and response header used to mark Inertia protocol requests.
pub const X_INERTIA: &str = "X-Inertia";

/// Request header containing the client's current asset version.
pub const X_INERTIA_VERSION: &str = "X-Inertia-Version";

/// Typed form of [`X_INERTIA`] for response header insertion.
pub const X_INERTIA_HEADER: HeaderName = HeaderName::from_static("x-inertia");

/// Typed form of [`X_INERTIA_VERSION`] for request header lookup.
pub const X_INERTIA_VERSION_HEADER: HeaderName = HeaderName::from_static("x-inertia-version");

/// Request header containing the component targeted by a partial reload.
pub const X_INERTIA_PARTIAL_COMPONENT: &str = "X-Inertia-Partial-Component";

/// Request header listing props to include in a partial reload.
pub const X_INERTIA_PARTIAL_DATA: &str = "X-Inertia-Partial-Data";

/// Request header listing props to exclude from a partial reload.
pub const X_INERTIA_PARTIAL_EXCEPT: &str = "X-Inertia-Partial-Except";

/// Request header listing props to reset on navigation.
pub const X_INERTIA_RESET: &str = "X-Inertia-Reset";

/// Request header identifying a validation error bag.
pub const X_INERTIA_ERROR_BAG: &str = "X-Inertia-Error-Bag";

/// Request header used by Inertia's infinite scroll protocol.
pub const X_INERTIA_INFINITE_SCROLL_MERGE_INTENT: &str = "X-Inertia-Infinite-Scroll-Merge-Intent";

/// Request header listing once-prop keys the client already has.
pub const X_INERTIA_EXCEPT_ONCE_PROPS: &str = "X-Inertia-Except-Once-Props";

/// Response header used with `409 Conflict` to force a full-page visit.
pub const X_INERTIA_LOCATION: &str = "X-Inertia-Location";

/// Typed form of [`X_INERTIA_LOCATION`] for response header insertion.
pub const X_INERTIA_LOCATION_HEADER: HeaderName = HeaderName::from_static("x-inertia-location");

/// Response header used with fragment redirects.
pub const X_INERTIA_REDIRECT: &str = "X-Inertia-Redirect";

/// Typed form of [`X_INERTIA_REDIRECT`] for response header insertion.
pub const X_INERTIA_REDIRECT_HEADER: HeaderName = HeaderName::from_static("x-inertia-redirect");

/// Response header used to separate HTML and JSON variants in caches.
pub const VARY: &str = "Vary";

/// Request header set to `prefetch` for Inertia prefetch requests.
pub const PURPOSE: &str = "Purpose";

/// Request header set to `no-cache` for Inertia reload requests.
pub const CACHE_CONTROL: &str = "Cache-Control";
