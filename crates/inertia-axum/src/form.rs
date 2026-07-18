//! Axum form extraction and validation backed by the core form model.

use crate::response::pending_response;
use axum::{
    body::to_bytes,
    extract::{FromRequest, OriginalUri, Request},
    response::{IntoResponse, Response},
};
use serde::de::DeserializeOwned;
use std::ops::{Deref, DerefMut};

pub use inertia_core::form::serialize_old_input;

/// Parsed Inertia form plus redirect metadata.
pub struct InertiaForm<T>(inertia_core::Form<T>);

impl<T> InertiaForm<T> {
    /// Returns the parsed value without validating it.
    pub fn into_inner(self) -> T {
        self.0.into_inner()
    }

    /// Applies application-defined validation.
    pub fn validate_with<F>(self, validate: F) -> Result<T, FormError>
    where
        F: FnOnce(&T) -> Result<(), inertia_core::Errors>,
    {
        self.0.validate_with(validate).map_err(FormError)
    }
}

impl<T> Deref for InertiaForm<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for InertiaForm<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// A typed form value that passed validation.
pub struct Validated<T>(pub T);

/// Axum rejection for form decoding or semantic validation.
#[derive(Debug)]
pub struct FormError(pub inertia_core::FormError);

impl std::fmt::Display for FormError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl std::error::Error for FormError {}

impl IntoResponse for FormError {
    fn into_response(self) -> Response {
        match self.0 {
            inertia_core::FormError::BadRequest(error) => {
                (http::StatusCode::BAD_REQUEST, error).into_response()
            }
            inertia_core::FormError::UnsupportedMediaType => (
                http::StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "InertiaForm supports JSON and URL-encoded bodies; multipart requires a separate extractor",
            )
                .into_response(),
            inertia_core::FormError::Validation(validation) => pending_response(
                inertia_core::PendingResponse::InvalidForm(validation),
            ),
        }
    }
}

impl<S, T> FromRequest<S> for InertiaForm<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Send + 'static,
{
    type Rejection = FormError;

    async fn from_request(request: Request, _state: &S) -> Result<Self, Self::Rejection> {
        let (parts, body) = request.into_parts();
        let uri = parts
            .extensions
            .get::<OriginalUri>()
            .map(|original| original.0.clone())
            .unwrap_or(parts.uri);
        let request = inertia_core::RequestParts::new(parts.method, uri, parts.headers);
        let bytes = to_bytes(body, 2 * 1024 * 1024)
            .await
            .map_err(|error| FormError(inertia_core::FormError::BadRequest(error.to_string())))?;
        inertia_core::Form::from_bytes(&request, &bytes)
            .map(Self)
            .map_err(FormError)
    }
}

impl<S, T> FromRequest<S> for Validated<T>
where
    S: Send + Sync,
    T: DeserializeOwned + inertia_core::Validate + Send + 'static,
{
    type Rejection = FormError;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        let form = InertiaForm::<T>::from_request(request, state).await?;
        inertia_core::Validated::from_form(form.0)
            .map(|validated| Self(validated.0))
            .map_err(FormError)
    }
}
