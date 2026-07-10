//! Framework-neutral redirect response models.
//!
//! Axum consumes these values in its rendering façade; keeping their storage
//! here preserves the root-level constructors and getters.

/// A server-initiated external location visit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Location {
    url: String,
}

impl Location {
    /// Creates an external location redirect to `url`.
    pub fn new<U: Into<String>>(url: U) -> Self {
        Self { url: url.into() }
    }

    /// Returns the destination URL.
    pub fn url(&self) -> &str {
        &self.url
    }
}

/// A method-aware redirect response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Redirect {
    url: String,
}

impl Redirect {
    /// Creates a redirect response to `url`.
    pub fn new<U: Into<String>>(url: U) -> Self {
        Self { url: url.into() }
    }

    /// Returns the redirect destination URL.
    pub fn url(&self) -> &str {
        &self.url
    }
}
