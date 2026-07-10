//! Shared Axum header, URI, and redirect-response helpers.
//!
//! Rendering and version middleware consume these private helpers so URI
//! validation and status/header semantics have one implementation.

use super::error::InertiaError;
use crate::{
    RequestContext, VARY, X_INERTIA, X_INERTIA_LOCATION_HEADER, X_INERTIA_REDIRECT_HEADER,
};
use axum::extract::OriginalUri;
use axum::http::header::{LOCATION, VARY as VARY_HEADER};
use axum::http::uri::Uri;
use axum::http::{HeaderMap, HeaderValue, Method, Request, StatusCode};
use axum::response::{IntoResponse, Response};
use fluent_uri::UriRef;

pub(crate) fn header<'headers>(headers: &'headers HeaderMap, name: &str) -> Option<&'headers str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

pub(crate) fn request_context(headers: &HeaderMap) -> RequestContext {
    RequestContext::from_header_fn(|name| header(headers, name))
}

pub(crate) fn add_vary_header(response: &mut Response) {
    let has_inertia = response
        .headers()
        .get_all(VARY_HEADER)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .any(|value| value.eq_ignore_ascii_case(X_INERTIA));

    if !has_inertia {
        response
            .headers_mut()
            .append(VARY, HeaderValue::from_static(X_INERTIA));
    }
}

pub(crate) fn is_write_method(method: &Method) -> bool {
    matches!(
        *method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    )
}

fn location_header(url: &str) -> Result<HeaderValue, InertiaError> {
    let (header, _has_fragment) = location_header_with_fragment(url)?;
    Ok(header)
}

fn location_header_with_fragment(url: &str) -> Result<(HeaderValue, bool), InertiaError> {
    let uri = UriRef::parse(url).map_err(InertiaError::invalid_uri)?;
    let has_fragment = uri.fragment().is_some();
    HeaderValue::from_str(url)
        .map_err(InertiaError::invalid_header)
        .map(|header| (header, has_fragment))
}

pub(crate) fn local_uri(uri: &Uri) -> Box<str> {
    uri.path_and_query()
        .map(|path_and_query| path_and_query.as_str().into())
        .unwrap_or_else(|| "/".into())
}

pub(crate) fn original_local_uri<B>(request: &Request<B>) -> &str {
    request
        .extensions()
        .get::<OriginalUri>()
        .map(|original_uri| &original_uri.0)
        .unwrap_or_else(|| request.uri())
        .path_and_query()
        .map_or("/", |path_and_query| path_and_query.as_str())
}

pub(crate) fn redirect_response(status: StatusCode, url: &str) -> Result<Response, InertiaError> {
    let mut response = status.into_response();
    response
        .headers_mut()
        .insert(LOCATION, location_header(url)?);
    add_vary_header(&mut response);
    Ok(response)
}

pub(crate) fn conflict_response(url: &str) -> Result<Response, InertiaError> {
    let mut response = StatusCode::CONFLICT.into_response();
    let (location, has_fragment) = location_header_with_fragment(url)?;
    let header = if has_fragment {
        X_INERTIA_REDIRECT_HEADER
    } else {
        X_INERTIA_LOCATION_HEADER
    };
    response.headers_mut().insert(header, location);
    add_vary_header(&mut response);
    Ok(response)
}
