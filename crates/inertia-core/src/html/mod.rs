//! HTML response context and serialization helpers.
//!
//! This module owns the framework-neutral context passed to server-side HTML
//! render callbacks. Serialization uses the neighboring script-safe formatter
//! and a preallocated byte buffer.

mod serializer;

use self::serializer::to_script_safe_json;
use bytes::Bytes;
use serde::{Serialize, Serializer};

/// Context passed to framework HTML response renderers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HtmlResponseContext {
    data_page: Bytes,
}

impl HtmlResponseContext {
    /// Creates a context from a serialized page object string.
    ///
    /// Framework integrations construct this with script-safe JSON. If you
    /// create it manually, make sure the value is safe for its target HTML
    /// context.
    pub fn new<D: Into<String>>(data_page: D) -> Self {
        Self {
            data_page: Bytes::from(data_page.into()),
        }
    }

    pub(crate) fn from_bytes(data_page: Bytes) -> Self {
        Self { data_page }
    }

    /// Returns the JSON-serialized Inertia page object.
    pub fn data_page(&self) -> &str {
        std::str::from_utf8(&self.data_page).expect("Inertia page JSON is always UTF-8")
    }

    #[cfg(feature = "ssr")]
    pub(crate) fn data_page_bytes(&self) -> Bytes {
        self.data_page.clone()
    }
}

impl Serialize for HtmlResponseContext {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        struct Representation<'a> {
            data_page: &'a str,
        }

        Representation {
            data_page: self.data_page(),
        }
        .serialize(serializer)
    }
}

#[doc(hidden)]
pub fn html_response_context<T>(page: &T) -> Result<HtmlResponseContext, serde_json::Error>
where
    T: Serialize + ?Sized,
{
    to_script_safe_json(page).map(HtmlResponseContext::from_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn script_safe_serializer_returns_bytes_without_changing_output() {
        let bytes = to_script_safe_json(&json!({"message": "safe"})).unwrap();
        assert_eq!(bytes, r#"{"message":"safe"}"#);
    }

    #[test]
    fn html_response_context_preserves_serialized_shape() {
        let context = html_response_context(&json!({"component": "Home"})).unwrap();
        assert_eq!(context.data_page(), r#"{"component":"Home"}"#);
        assert_eq!(
            serde_json::to_value(&context).unwrap(),
            json!({"data_page": context.data_page()})
        );
        #[cfg(feature = "ssr")]
        assert_eq!(context.data_page_bytes(), context.data_page().as_bytes());
    }

    #[test]
    fn unsafe_script_sequences_remain_escaped() {
        let context =
            html_response_context(&json!({"unsafe": "</script><>&\u{2028}\u{2029}"})).unwrap();
        let page = context.data_page();
        assert!(!page.contains("</script>"));
        assert!(!page.contains('<'));
        assert!(!page.contains('>'));
        assert!(!page.contains('&'));
        assert!(!page.contains('\u{2028}'));
        assert!(!page.contains('\u{2029}'));
        assert!(page.contains(r"\u003C/script\u003E\u003C\u003E\u0026\u2028\u2029"));
    }
}
