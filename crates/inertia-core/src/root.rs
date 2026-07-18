//! Application-wide root document rendering.

use std::{convert::Infallible, fmt, sync::Arc};

#[cfg(feature = "askama")]
mod askama;
mod template;

#[cfg(feature = "askama")]
pub(crate) use askama::AskamaRootView;
#[cfg(feature = "askama")]
pub use askama::{AskamaRoot, AskamaRootContext};
pub(crate) use template::CompiledRootTemplate;

/// Pre-rendered application asset markup.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AssetTags(String);

impl AssetTags {
    /// Wraps trusted, pre-rendered asset markup from an [`AssetProvider`](crate::AssetProvider).
    pub fn new(markup: String) -> Self {
        Self(markup)
    }
    pub(crate) fn empty() -> Self {
        Self(String::new())
    }

    /// Returns the trusted markup as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AssetTags {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Trusted markup returned for the document head by the configured SSR backend.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HeadMarkup(String);

impl HeadMarkup {
    pub(crate) fn empty() -> Self {
        Self(String::new())
    }

    #[cfg(feature = "ssr")]
    pub(crate) fn from_fragments(fragments: impl IntoIterator<Item = String>) -> Self {
        Self(fragments.into_iter().collect())
    }

    /// Returns the trusted markup as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[cfg(test)]
    pub(crate) fn for_test(markup: &str) -> Self {
        Self(markup.to_owned())
    }
}

impl fmt::Display for HeadMarkup {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Pre-rendered, script-safe Inertia mount markup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MountMarkup(String);

impl MountMarkup {
    pub(crate) fn csr(page: &str) -> Self {
        Self(format!(
            "<script data-page=\"app\" type=\"application/json\">{page}</script><div id=\"app\"></div>"
        ))
    }

    #[cfg(feature = "ssr")]
    pub(crate) fn ssr(body: String) -> Self {
        Self(body)
    }

    /// Returns the trusted markup as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[cfg(test)]
    pub(crate) fn for_test(markup: &str) -> Self {
        Self(markup.to_owned())
    }
}

impl fmt::Display for MountMarkup {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Safe values available to an application root renderer.
#[derive(Clone, Copy)]
pub struct RootContext<'a> {
    title: Option<&'a str>,
    locale: Option<&'a str>,
    assets: &'a AssetTags,
    head: &'a HeadMarkup,
    mount: &'a MountMarkup,
    nonce: Option<&'a str>,
}

impl<'a> RootContext<'a> {
    pub(crate) fn new(assets: &'a AssetTags, head: &'a HeadMarkup, mount: &'a MountMarkup) -> Self {
        Self {
            title: None,
            locale: None,
            assets,
            head,
            mount,
            nonce: None,
        }
    }

    /// Returns the optional page title.
    pub fn title(&self) -> Option<&str> {
        self.title
    }
    /// Returns the optional page locale.
    pub fn locale(&self) -> Option<&str> {
        self.locale
    }
    /// Returns pre-rendered, safe asset markup.
    pub fn assets(&self) -> &AssetTags {
        self.assets
    }
    /// Returns trusted markup generated for the document head by SSR.
    pub fn head(&self) -> &HeadMarkup {
        self.head
    }
    /// Returns pre-rendered, script-safe mount markup.
    pub fn mount(&self) -> &MountMarkup {
        self.mount
    }
    /// Returns the optional content-security-policy nonce.
    pub fn nonce(&self) -> Option<&str> {
        self.nonce
    }

    #[cfg(feature = "askama")]
    pub(crate) fn fragment_len(&self) -> usize {
        self.assets.as_str().len() + self.head.as_str().len() + self.mount.as_str().len()
    }
}

/// Renders the application-wide initial HTML document.
///
/// For the built-in root, use
/// [`InertiaApp::default_root`](crate::InertiaApp::default_root) or
/// `InertiaApp::vite(...)`.
///
/// For a startup-compiled marker template, use
/// [`InertiaAppBuilder::root_template`](crate::InertiaAppBuilder::root_template).
///
/// With the `askama` feature enabled, typed Askama templates can be configured
/// with `InertiaAppBuilder::askama_root(...)`.
///
/// Custom implementations control their own rendering strategy and
/// performance.
pub trait RootView: Clone + Send + Sync + 'static {
    /// Rendering failure.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Renders a complete HTML document from safe pre-rendered fragments.
    fn render(&self, context: RootContext<'_>) -> Result<String, Self::Error>;
}

pub(crate) trait ErasedRootView: Send + Sync {
    fn render(
        &self,
        context: RootContext<'_>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>>;
}

impl<T: RootView> ErasedRootView for T {
    fn render(
        &self,
        context: RootContext<'_>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        RootView::render(self, context).map_err(|error| Box::new(error) as _)
    }
}

#[derive(Clone, Default)]
pub(crate) struct DefaultRoot;

const DEFAULT_ROOT_PREFIX: &str = "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">";
const DEFAULT_ROOT_BODY: &str = "</head><body>";
const DEFAULT_ROOT_SUFFIX: &str = "</body></html>";

impl RootView for DefaultRoot {
    type Error = Infallible;

    fn render(&self, context: RootContext<'_>) -> Result<String, Self::Error> {
        let capacity = DEFAULT_ROOT_PREFIX.len()
            + context.assets().as_str().len()
            + context.head().as_str().len()
            + DEFAULT_ROOT_BODY.len()
            + context.mount().as_str().len()
            + DEFAULT_ROOT_SUFFIX.len();
        let mut output = String::with_capacity(capacity);

        output.push_str(DEFAULT_ROOT_PREFIX);
        output.push_str(context.assets().as_str());
        output.push_str(context.head().as_str());
        output.push_str(DEFAULT_ROOT_BODY);
        output.push_str(context.mount().as_str());
        output.push_str(DEFAULT_ROOT_SUFFIX);

        Ok(output)
    }
}

pub(crate) type SharedRootView = Arc<dyn ErasedRootView>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csr_mount_contains_page_script_and_empty_app_element() {
        let mount = MountMarkup::csr(r#"{"component":"Home"}"#);
        assert_eq!(
            mount.to_string(),
            r#"<script data-page="app" type="application/json">{"component":"Home"}</script><div id="app"></div>"#
        );
    }

    #[test]
    #[cfg(feature = "ssr")]
    fn ssr_mount_preserves_backend_body_exactly() {
        let body = r#"<script data-page="app">{}</script><div id="app" data-server-rendered="true">rendered</div>"#;
        assert_eq!(MountMarkup::ssr(body.to_owned()).to_string(), body);
    }

    #[test]
    #[cfg(feature = "ssr")]
    fn default_root_places_ssr_head_inside_head_element() {
        let assets = AssetTags::empty();
        let head = HeadMarkup::from_fragments([
            "<title>SSR</title>".to_owned(),
            "<meta name=\"ssr\">".to_owned(),
        ]);
        let mount = MountMarkup::ssr("<div id=\"app\">rendered</div>".to_owned());
        let html =
            RootView::render(&DefaultRoot, RootContext::new(&assets, &head, &mount)).unwrap();
        assert!(html.contains("<head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>SSR</title><meta name=\"ssr\"></head>"));
        assert_eq!(html.matches("id=\"app\"").count(), 1);
    }

    #[test]
    fn default_root_uses_empty_head_for_csr() {
        let assets = AssetTags::empty();
        let head = HeadMarkup::empty();
        let mount = MountMarkup::csr("{}");
        let html =
            RootView::render(&DefaultRoot, RootContext::new(&assets, &head, &mount)).unwrap();
        assert!(html.contains("initial-scale=1\"></head>"));
    }

    #[test]
    fn default_root_output_is_byte_compatible() {
        let assets = AssetTags::new("<script src=\"/app.js\"></script>".to_owned());
        let head = HeadMarkup::for_test("<title>SSR</title>");
        let mount = MountMarkup::for_test("<div id=\"app\">rendered</div>");
        let html =
            RootView::render(&DefaultRoot, RootContext::new(&assets, &head, &mount)).unwrap();

        assert_eq!(
            html,
            "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><script src=\"/app.js\"></script><title>SSR</title></head><body><div id=\"app\">rendered</div></body></html>"
        );
        assert_eq!(html.capacity(), html.len());
    }
}
