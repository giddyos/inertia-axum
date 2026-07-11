//! Application-wide root document rendering.

use std::{convert::Infallible, fmt, sync::Arc};

/// Pre-rendered application asset markup.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AssetTags(String);

impl AssetTags {
    pub(crate) fn empty() -> Self {
        Self(String::new())
    }
}

impl fmt::Display for AssetTags {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Pre-rendered, script-safe Inertia mount markup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MountMarkup(String);

impl MountMarkup {
    pub(crate) fn new(page: &str) -> Self {
        Self(format!(
            "<script data-page=\"app\" type=\"application/json\">{page}</script><div id=\"app\"></div>"
        ))
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
    mount: &'a MountMarkup,
    nonce: Option<&'a str>,
}

impl<'a> RootContext<'a> {
    pub(crate) fn new(assets: &'a AssetTags, mount: &'a MountMarkup) -> Self {
        Self {
            title: None,
            locale: None,
            assets,
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
    /// Returns pre-rendered, script-safe mount markup.
    pub fn mount(&self) -> &MountMarkup {
        self.mount
    }
    /// Returns the optional content-security-policy nonce.
    pub fn nonce(&self) -> Option<&str> {
        self.nonce
    }
}

/// Renders the application-wide initial HTML document.
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

impl RootView for DefaultRoot {
    type Error = Infallible;

    fn render(&self, context: RootContext<'_>) -> Result<String, Self::Error> {
        Ok(format!(
            "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">{}</head><body>{}</body></html>",
            context.assets(), context.mount()
        ))
    }
}

pub(crate) type SharedRootView = Arc<dyn ErasedRootView>;
