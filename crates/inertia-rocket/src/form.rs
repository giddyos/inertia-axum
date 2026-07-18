//! Rocket form data guards and validation responses backed by the core form model.

use crate::{boundary, response::pending_response};
use rocket::{
    Request,
    data::{self, Data, FromData},
    http::Status,
    outcome::Outcome,
    response::{Responder, Response},
};
use std::{
    fmt,
    ops::{Deref, DerefMut},
    sync::Mutex,
};

pub use inertia_core::form::serialize_old_input;

/// Parsed Inertia form plus redirect metadata.
pub struct InertiaForm<T>(inertia_core::Form<T>);

impl<T> InertiaForm<T> {
    /// Returns the parsed value without validating it.
    pub fn into_inner(self) -> T {
        self.0.into_inner()
    }

    /// Applies application-defined validation.
    pub fn validate_with<F>(self, validate: F) -> std::result::Result<T, FormError>
    where
        F: FnOnce(&T) -> std::result::Result<(), inertia_core::Errors>,
    {
        self.0.validate_with(validate).map_err(FormError::from)
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

/// Rocket rejection for form decoding or semantic validation.
pub struct FormError(FormErrorKind);

enum FormErrorKind {
    BadRequest(String),
    UnsupportedMediaType,
    Validation(Mutex<Option<inertia_core::PendingResponse>>),
}

impl From<inertia_core::FormError> for FormError {
    fn from(error: inertia_core::FormError) -> Self {
        let kind = match error {
            inertia_core::FormError::BadRequest(error) => FormErrorKind::BadRequest(error),
            inertia_core::FormError::UnsupportedMediaType => FormErrorKind::UnsupportedMediaType,
            inertia_core::FormError::Validation(validation) => FormErrorKind::Validation(
                Mutex::new(Some(inertia_core::PendingResponse::InvalidForm(validation))),
            ),
        };
        Self(kind)
    }
}

impl fmt::Debug for FormError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("FormError")
            .field(&self.to_string())
            .finish()
    }
}

impl fmt::Display for FormError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            FormErrorKind::BadRequest(error) => {
                write!(formatter, "invalid Inertia form body: {error}")
            }
            FormErrorKind::UnsupportedMediaType => formatter.write_str(
                "InertiaForm supports application/json and application/x-www-form-urlencoded; use a separate multipart data guard for file uploads",
            ),
            FormErrorKind::Validation(_) => {
                formatter.write_str("Inertia form validation failed")
            }
        }
    }
}

impl std::error::Error for FormError {}

impl<'r, 'o: 'r> Responder<'r, 'o> for FormError {
    fn respond_to(self, request: &'r Request<'_>) -> rocket::response::Result<'o> {
        match self.0 {
            FormErrorKind::BadRequest(error) => Response::build_from(error.respond_to(request)?)
                .status(Status::BadRequest)
                .ok(),
            FormErrorKind::UnsupportedMediaType => Response::build_from(
                "InertiaForm supports JSON and URL-encoded bodies; multipart requires a separate data guard"
                    .respond_to(request)?,
            )
            .status(Status::UnsupportedMediaType)
            .ok(),
            FormErrorKind::Validation(pending) => {
                let pending = pending
                    .into_inner()
                    .expect("Rocket validation-response lock poisoned")
                    .ok_or(Status::InternalServerError)?;
                pending_response(request, pending)
            }
        }
    }
}

#[rocket::async_trait]
impl<'r, T> FromData<'r> for InertiaForm<T>
where
    T: rocket::serde::de::DeserializeOwned,
{
    type Error = FormError;

    async fn from_data(request: &'r Request<'_>, data: Data<'r>) -> data::Outcome<'r, Self> {
        let parts = match boundary::request_parts(request) {
            Ok(parts) => parts,
            Err(error) => {
                return Outcome::Error((
                    Status::BadRequest,
                    FormError(FormErrorKind::BadRequest(error)),
                ));
            }
        };
        let bytes = match Vec::<u8>::from_data(request, data).await {
            Outcome::Success(bytes) => bytes,
            Outcome::Error((status, error)) => {
                return Outcome::Error((
                    status,
                    FormError(FormErrorKind::BadRequest(error.to_string())),
                ));
            }
            Outcome::Forward((data, status)) => return Outcome::Forward((data, status)),
        };
        match inertia_core::Form::from_bytes(&parts, &bytes) {
            Ok(form) => Outcome::Success(Self(form)),
            Err(error) => {
                let error = FormError::from(error);
                let status = match error.0 {
                    FormErrorKind::BadRequest(_) => Status::BadRequest,
                    FormErrorKind::UnsupportedMediaType => Status::UnsupportedMediaType,
                    FormErrorKind::Validation(_) => Status::InternalServerError,
                };
                Outcome::Error((status, error))
            }
        }
    }
}

#[rocket::async_trait]
impl<'r, T> FromData<'r> for Validated<T>
where
    T: rocket::serde::de::DeserializeOwned + inertia_core::Validate,
{
    type Error = FormError;

    async fn from_data(request: &'r Request<'_>, data: Data<'r>) -> data::Outcome<'r, Self> {
        match InertiaForm::<T>::from_data(request, data).await {
            Outcome::Success(form) => match inertia_core::Validated::from_form(form.0) {
                Ok(validated) => Outcome::Success(Self(validated.0)),
                Err(error) => Outcome::Error((Status::InternalServerError, FormError::from(error))),
            },
            Outcome::Error(error) => Outcome::Error(error),
            Outcome::Forward(forward) => Outcome::Forward(forward),
        }
    }
}
