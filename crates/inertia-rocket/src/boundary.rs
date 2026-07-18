//! HTTP conversion between Rocket and the framework-neutral core.

use inertia_core::RequestParts;
use rocket::Request;

pub(crate) fn request_parts(request: &Request<'_>) -> Result<RequestParts, String> {
    let method = request
        .method()
        .as_str()
        .parse()
        .map_err(|error| format!("invalid request method at Rocket boundary: {error}"))?;
    let uri = request
        .uri()
        .to_string()
        .parse()
        .map_err(|error| format!("invalid request URI at Rocket boundary: {error}"))?;
    let mut headers = http::HeaderMap::new();
    for header in request.headers().iter() {
        let name = http::HeaderName::from_bytes(header.name().as_str().as_bytes())
            .map_err(|error| format!("invalid request header name at Rocket boundary: {error}"))?;
        let value = http::HeaderValue::from_bytes(header.value().as_bytes())
            .map_err(|error| format!("invalid request header value at Rocket boundary: {error}"))?;
        if is_inertia_protocol_header(&name) && value.to_str().is_err() {
            return Err(format!(
                "non-UTF-8 Inertia protocol header at Rocket boundary: {name}"
            ));
        }
        headers.append(name, value);
    }
    Ok(RequestParts::new(method, uri, headers))
}

fn is_inertia_protocol_header(name: &http::HeaderName) -> bool {
    let name = name.as_str();
    name.starts_with("x-inertia") || matches!(name, "accept" | "purpose" | "x-requested-with")
}
