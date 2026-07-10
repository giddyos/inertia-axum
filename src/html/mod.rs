//! HTML response context and serialization helpers.
//!
//! This module owns the framework-neutral context passed to server-side HTML
//! render callbacks. Serialization uses the neighboring script-safe formatter
//! and a preallocated byte buffer.

mod serializer;

use self::serializer::to_script_safe_json;
use serde::Serialize;

/// Context passed to framework HTML response renderers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct HtmlResponseContext {
    data_page: String,
}

impl HtmlResponseContext {
    /// Creates a context from a serialized page object string.
    ///
    /// Framework integrations construct this with script-safe JSON. If you
    /// create it manually, make sure the value is safe for its target HTML
    /// context.
    pub fn new<D: Into<String>>(data_page: D) -> Self {
        Self {
            data_page: data_page.into(),
        }
    }

    /// Returns the JSON-serialized Inertia page object.
    pub fn data_page(&self) -> &str {
        &self.data_page
    }
}

pub(crate) fn html_response_context<T>(page: &T) -> Result<HtmlResponseContext, serde_json::Error>
where
    T: Serialize + ?Sized,
{
    to_script_safe_json(page).map(HtmlResponseContext::new)
}
