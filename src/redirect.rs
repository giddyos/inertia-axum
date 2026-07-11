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
    /// Creates an external location visit.
    pub fn external<U: Into<String>>(url: U) -> Self {
        Self::new(url)
    }
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
    use_referrer: bool,
    pub(crate) flash: serde_json::Map<String, serde_json::Value>,
}

impl Redirect {
    /// Creates a redirect response to `url`.
    pub fn new<U: Into<String>>(url: U) -> Self {
        Self {
            url: url.into(),
            use_referrer: false,
            flash: serde_json::Map::new(),
        }
    }

    /// Creates a redirect to a concrete destination.
    pub fn to<U: Into<String>>(url: U) -> Self {
        Self::new(url)
    }

    /// Creates a redirect to the request referrer, falling back to `/`.
    pub fn back() -> Self {
        Self {
            url: "/".to_owned(),
            use_referrer: true,
            flash: serde_json::Map::new(),
        }
    }

    /// Creates a redirect to the request referrer with an explicit fallback.
    pub fn back_or<U: Into<String>>(fallback: U) -> Self {
        Self {
            url: fallback.into(),
            use_referrer: true,
            flash: serde_json::Map::new(),
        }
    }

    /// Returns the redirect destination URL.
    pub fn url(&self) -> &str {
        &self.url
    }

    pub(crate) fn resolve<'a>(&'a self, referer: Option<&'a str>) -> &'a str {
        if self.use_referrer {
            referer.unwrap_or_else(|| self.url())
        } else {
            self.url()
        }
    }

    /// Flashes a serialized value to the page reached by this redirect.
    pub fn flash(mut self, key: impl Into<String>, value: impl serde::Serialize) -> Self {
        self.flash.insert(
            key.into(),
            serde_json::to_value(value).expect("Redirect flash serialization failed"),
        );
        self
    }
}

impl axum::response::IntoResponse for Redirect {
    fn into_response(self) -> axum::response::Response {
        crate::response::pending_response(crate::PendingResponse::Redirect(self))
    }
}

impl axum::response::IntoResponse for Location {
    fn into_response(self) -> axum::response::Response {
        crate::response::pending_response(crate::PendingResponse::Location(self))
    }
}
