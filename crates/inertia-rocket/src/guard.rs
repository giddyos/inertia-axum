//! Rocket request guards backed by the framework-neutral core.

use crate::{
    DynamicPage, Error, Location, PendingPage, Redirect, Response, Result, boundary,
    response::EarlyResponseSlot,
};
use rocket::{
    Request,
    http::Status,
    request::{FromRequest, Outcome},
};
use std::{marker::PhantomData, ops::Deref};

/// Request-aware asynchronous Inertia rendering façade.
pub struct Inertia<'r> {
    prepared: inertia_core::PreparedRequest,
    visit: inertia_core::Visit,
    marker: PhantomData<&'r ()>,
}

impl Inertia<'_> {
    /// Constructs a legacy framework-neutral page value.
    pub fn response<C: Into<String>, T>(component: C, props: T) -> inertia_core::Inertia<T> {
        inertia_core::Inertia::response(component, props)
    }

    /// Starts the legacy advanced page builder.
    pub fn page(component: impl Into<String>) -> inertia_core::InertiaPageBuilder {
        inertia_core::Inertia::page(component)
    }

    /// Builds and asynchronously finalizes a page.
    pub async fn render(
        self,
        component: impl Into<String>,
        props: impl rocket::serde::Serialize,
    ) -> Result {
        let value = inertia_core::__private::to_value(props).map_err(Error::internal)?;
        let mut page = DynamicPage::new(component);
        match value {
            inertia_core::__private::Value::Object(values) => {
                for (key, value) in values {
                    page = page.prop(key, value);
                }
            }
            value => page = page.prop("value", value),
        }
        self.finalize(inertia_core::PendingResponse::Page(Box::new(
            page.into_pending_page().into_core(),
        )))
        .await
    }

    /// Asynchronously finalizes a derived typed page.
    pub async fn render_typed(self, page: impl inertia_core::InertiaPage) -> Result {
        self.finalize(inertia_core::PendingResponse::Page(Box::new(
            PendingPage::typed(page).into_core(),
        )))
        .await
    }

    /// Asynchronously finalizes a method-aware redirect.
    pub async fn to(self, url: impl Into<String>) -> Result {
        self.finalize(inertia_core::PendingResponse::Redirect(
            Redirect::to(url).into_core(),
        ))
        .await
    }

    /// Asynchronously finalizes an external location visit.
    pub async fn external(self, url: impl Into<String>) -> Result {
        self.finalize(inertia_core::PendingResponse::Location(
            Location::external(url).into_core(),
        ))
        .await
    }

    /// Returns the parsed framework-neutral visit.
    pub fn visit(&self) -> &inertia_core::Visit {
        &self.visit
    }

    async fn finalize(self, pending: inertia_core::PendingResponse) -> Result {
        #[cfg(feature = "ssr")]
        let response = self.prepared.finalize_with_ssr(pending, None).await;
        #[cfg(not(feature = "ssr"))]
        let response = self.prepared.finalize(pending).await;
        Ok(Response(response))
    }
}

impl Deref for Inertia<'_> {
    type Target = inertia_core::Visit;

    fn deref(&self) -> &Self::Target {
        &self.visit
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Inertia<'r> {
    type Error = String;

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let Some(app) = request.rocket().state::<inertia_core::InertiaApp>() else {
            return Outcome::Error((
                Status::InternalServerError,
                "inertia-rocket is not installed; attach InertiaFairing".to_owned(),
            ));
        };
        let parts = match boundary::request_parts(request) {
            Ok(parts) => parts,
            Err(error) => return Outcome::Error((Status::BadRequest, error)),
        };
        match app.prepare_request(parts, None).await {
            Ok(inertia_core::VersionCheck::Proceed(prepared)) => {
                let prepared = *prepared;
                let visit = prepared.visit().clone();
                Outcome::Success(Self {
                    prepared,
                    visit,
                    marker: PhantomData,
                })
            }
            Ok(inertia_core::VersionCheck::Mismatch(response)) => {
                request
                    .local_cache(EarlyResponseSlot::default)
                    .store(response);
                Outcome::Error((
                    Status::Conflict,
                    "Inertia asset version mismatch".to_owned(),
                ))
            }
            Err(error) => {
                request
                    .local_cache(EarlyResponseSlot::default)
                    .store(error.into_response());
                Outcome::Error((
                    Status::InternalServerError,
                    "failed to prepare Inertia request".to_owned(),
                ))
            }
        }
    }
}

/// Parsed visit guard that remains available without an installed app.
#[derive(Clone, Debug)]
pub struct Visit(inertia_core::Visit);

impl Deref for Visit {
    type Target = inertia_core::Visit;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Visit {
    type Error = String;

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        match boundary::request_parts(request) {
            Ok(parts) => Outcome::Success(Self(inertia_core::Visit::from(parts))),
            Err(error) => Outcome::Error((Status::BadRequest, error)),
        }
    }
}
