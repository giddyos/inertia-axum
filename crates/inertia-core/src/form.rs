//! Redirect-based Inertia form extraction and validation.

use crate::{RequestContext, RequestParts};
use http::header::{CONTENT_TYPE, REFERER};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use std::{
    fmt,
    ops::{Deref, DerefMut},
};

/// Standard field-to-message validation errors.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Errors(Map<String, Value>);
impl Errors {
    /// Creates an empty error map.
    pub fn new() -> Self {
        Self::default()
    }
    /// Inserts or replaces a field error.
    pub fn add(&mut self, field: impl Into<String>, message: impl Into<String>) {
        self.0.insert(field.into(), Value::String(message.into()));
    }
    /// Creates an error map containing one field error.
    pub fn field(field: impl Into<String>, message: impl Into<String>) -> Self {
        let mut errors = Self::new();
        errors.add(field, message);
        errors
    }
    /// Returns whether no field errors are present.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    /// Converts this map to its JSON representation.
    pub fn into_value(self) -> Value {
        Value::Object(self.0)
    }
}

/// Local validation contract implemented by derives or application code.
pub trait Validate {
    /// Validates this form value.
    fn validate(&self) -> Result<(), Errors>;
    /// Returns the derive-level fallback error bag.
    fn error_bag() -> Option<&'static str> {
        None
    }
    /// Returns explicitly enabled, redacted old input.
    fn old_input(&self) -> Option<Value> {
        None
    }
}

/// Parsed form plus request metadata for lower-level custom validation.
pub struct InertiaForm<T> {
    input: T,
    bag: Option<Box<str>>,
    back: Box<str>,
}
impl<T> InertiaForm<T> {
    /// Returns the parsed value without performing validation.
    pub fn into_inner(self) -> T {
        self.input
    }

    /// Parses a supported request body using framework-neutral request parts.
    pub fn from_bytes(request: &RequestParts, bytes: &[u8]) -> Result<Self, FormError>
    where
        T: DeserializeOwned,
    {
        let context = RequestContext::from_header_fn(|name| {
            request
                .headers()
                .get(name)
                .and_then(|value| value.to_str().ok())
        });
        let bag = context.error_bag().map(Into::into);
        let back = request
            .headers()
            .get(REFERER)
            .and_then(|value| value.to_str().ok())
            .map_or_else(
                || request.uri().path().to_owned().into_boxed_str(),
                Into::into,
            );
        let content_type = request
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .split(';')
            .next()
            .unwrap_or("")
            .trim();
        let input = match content_type {
            "application/json" => serde_json::from_slice(bytes)
                .map_err(|error| FormError::BadRequest(error.to_string()))?,
            "application/x-www-form-urlencoded" => serde_urlencoded::from_bytes(bytes)
                .map_err(|error| FormError::BadRequest(error.to_string()))?,
            _ => return Err(FormError::UnsupportedMediaType),
        };
        Ok(Self { input, bag, back })
    }
    /// Applies application-defined validation to the parsed value.
    pub fn validate_with<F>(self, validate: F) -> Result<T, FormError>
    where
        F: FnOnce(&T) -> Result<(), Errors>,
    {
        match validate(&self.input) {
            Ok(()) => Ok(self.input),
            Err(errors) => Err(FormError::validation(errors, self.bag, self.back, None)),
        }
    }
}
impl<T> Deref for InertiaForm<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.input
    }
}
impl<T> DerefMut for InertiaForm<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.input
    }
}

/// A typed form value that passed validation.
pub struct Validated<T>(pub T);

impl<T: Validate> Validated<T> {
    /// Validates an already parsed Inertia form.
    pub fn from_form(form: InertiaForm<T>) -> Result<Self, FormError> {
        match form.input.validate() {
            Ok(()) => Ok(Self(form.input)),
            Err(errors) => {
                let old_input = form.input.old_input();
                let bag = form.bag.or_else(|| T::error_bag().map(Into::into));
                Err(FormError::validation(errors, bag, form.back, old_input))
            }
        }
    }
}

#[derive(Debug)]
/// Form extraction or redirect-based validation failure.
pub enum FormError {
    /// The supported request body could not be decoded.
    BadRequest(String),
    /// The request content type requires a separate extractor.
    UnsupportedMediaType,
    /// Semantic validation failed and must be transported through a redirect.
    Validation(crate::response::PendingValidation),
}
impl FormError {
    fn validation(
        errors: Errors,
        bag: Option<Box<str>>,
        back: Box<str>,
        old_input: Option<Value>,
    ) -> Self {
        let errors = if let Some(bag) = bag {
            let mut scoped = Map::new();
            scoped.insert(bag.into(), errors.into_value());
            Value::Object(scoped)
        } else {
            errors.into_value()
        };
        Self::Validation(crate::response::PendingValidation {
            errors,
            old_input,
            back,
        })
    }
}
impl fmt::Display for FormError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadRequest(error) => write!(f, "invalid Inertia form body: {error}"),
            Self::UnsupportedMediaType => f.write_str("InertiaForm supports application/json and application/x-www-form-urlencoded; use a separate multipart extractor for file uploads"),
            Self::Validation(_) => f.write_str("Inertia form validation failed"),
        }
    }
}
impl FormError {
    /// Converts a validation failure into a pending core response.
    pub fn into_pending(self) -> Option<crate::PendingResponse> {
        match self {
            Self::Validation(validation) => Some(crate::PendingResponse::InvalidForm(validation)),
            Self::BadRequest(_) | Self::UnsupportedMediaType => None,
        }
    }
}

#[doc(hidden)]
pub fn serialize_old_input(
    fields: impl IntoIterator<Item = (&'static str, Result<Value, serde_json::Error>)>,
) -> Value {
    let mut values = Map::new();
    for (name, value) in fields {
        if let Ok(value) = value {
            values.insert(name.to_owned(), value);
        }
    }
    Value::Object(values)
}
