//! Axum extractors backed by the core request projection.

use crate::{Location, PendingPage, Redirect};
use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use serde::Serialize;
use std::{convert::Infallible, ops::Deref};

/// Request-aware Inertia rendering façade.
pub struct Inertia {
    visit: inertia_core::Visit,
}

impl Inertia {
    /// Constructs a legacy framework-neutral page object.
    pub fn response<C: Into<String>, T>(component: C, props: T) -> inertia_core::Inertia<T> {
        inertia_core::Inertia::response(component, props)
    }

    /// Starts the legacy advanced page builder.
    pub fn page(component: impl Into<String>) -> inertia_core::InertiaPageBuilder {
        inertia_core::Inertia::page(component)
    }

    /// Constructs a legacy external location value.
    pub fn location(url: impl Into<String>) -> inertia_core::Location {
        inertia_core::Inertia::location(url)
    }

    /// Constructs a legacy method-aware redirect value.
    pub fn redirect(url: impl Into<String>) -> inertia_core::Redirect {
        inertia_core::Inertia::redirect(url)
    }

    /// Builds a pending page from a component name and serializable props.
    pub fn render(self, component: impl Into<String>, props: impl Serialize) -> PendingPage {
        let value = serde_json::to_value(props).expect("Inertia page props must serialize");
        let mut page = crate::DynamicPage::new(component);
        match value {
            serde_json::Value::Object(values) => {
                for (key, value) in values {
                    page = page.prop(key, value);
                }
            }
            value => {
                page = page.prop("value", value);
            }
        }
        page.into_pending_page()
    }

    /// Wraps a derived typed page for Axum response conversion.
    pub fn render_typed(self, page: impl inertia_core::InertiaPage) -> PendingPage {
        PendingPage::typed(page)
    }

    /// Creates a method-aware redirect.
    pub fn to(self, url: impl Into<String>) -> Redirect {
        Redirect::to(url)
    }

    /// Creates an external Inertia location visit.
    pub fn external(self, url: impl Into<String>) -> Location {
        Location::external(url)
    }

    /// Returns the parsed visit.
    pub fn visit(&self) -> &inertia_core::Visit {
        &self.visit
    }
}

impl<S> FromRequestParts<S> for Inertia
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<inertia_core::Visit>()
            .cloned()
            .map(|visit| Self { visit })
            .ok_or_else(missing_inertia_app)
    }
}

fn missing_inertia_app() -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        "InertiaApp is not installed. Call RouterInertiaExt::with_inertia(app) on the Axum router.",
    )
        .into_response()
}

/// Axum extractor wrapper for advanced visit inspection.
#[derive(Clone, Debug)]
pub struct Visit(inertia_core::Visit);

impl Deref for Visit {
    type Target = inertia_core::Visit;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<S> FromRequestParts<S> for Visit
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let visit = parts
            .extensions
            .get::<inertia_core::Visit>()
            .cloned()
            .unwrap_or_else(|| {
                let request = inertia_core::RequestParts::new(
                    parts.method.clone(),
                    parts.uri.clone(),
                    parts.headers.clone(),
                );
                inertia_core::Visit::from(request)
            });
        Ok(Self(visit))
    }
}
