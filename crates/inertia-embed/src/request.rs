use crate::{
    EmbeddedFrontend, EmbeddedStorage,
    cache::{IMMUTABLE, REVALIDATE, etag_matches},
};
use http::{
    HeaderMap, HeaderValue, Method, StatusCode,
    header::{
        ACCEPT_RANGES, ALLOW, CACHE_CONTROL, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE, ETAG,
        VARY,
    },
};
use inertia_core::{AssetBody, AssetRequest, AssetResponse};

pub(crate) fn respond(
    frontend: &'static EmbeddedFrontend,
    request: AssetRequest<'_>,
) -> Option<AssetResponse> {
    if request.method != Method::GET && request.method != Method::HEAD {
        let mut headers = HeaderMap::new();
        headers.insert(ALLOW, HeaderValue::from_static("GET, HEAD"));
        return Some(AssetResponse {
            status: StatusCode::METHOD_NOT_ALLOWED,
            headers,
            body: AssetBody::Empty,
        });
    }
    let asset = frontend.find(request.path)?;
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static(asset.content_type));
    headers.insert(ETAG, HeaderValue::from_static(asset.etag));
    headers.insert(ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    headers.insert(
        CACHE_CONTROL,
        HeaderValue::from_static(if asset.immutable {
            IMMUTABLE
        } else {
            REVALIDATE
        }),
    );
    if let Some(encoding) = asset.encoding {
        headers.insert(CONTENT_ENCODING, HeaderValue::from_static(encoding));
        headers.insert(VARY, HeaderValue::from_static("Accept-Encoding"));
    }
    if etag_matches(request.headers, asset.etag) {
        return Some(AssetResponse {
            status: StatusCode::NOT_MODIFIED,
            headers,
            body: AssetBody::Empty,
        });
    }
    headers.insert(
        CONTENT_LENGTH,
        HeaderValue::from_str(&asset.uncompressed_len().to_string()).ok()?,
    );
    let body = if request.method == Method::HEAD {
        AssetBody::Empty
    } else {
        match asset.storage {
            EmbeddedStorage::Identity => AssetBody::Static(asset.bytes),
            EmbeddedStorage::Brotli { .. } => match EmbeddedFrontend::response_bytes(asset) {
                Ok(bytes) => AssetBody::Bytes(bytes),
                Err(_) => {
                    return Some(AssetResponse {
                        status: StatusCode::INTERNAL_SERVER_ERROR,
                        headers: HeaderMap::new(),
                        body: AssetBody::Empty,
                    });
                }
            },
        }
    };
    Some(AssetResponse {
        status: StatusCode::OK,
        headers,
        body,
    })
}
