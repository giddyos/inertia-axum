//! Optional server-side rendering support.

mod client;
mod config;
mod error;
mod policy;

pub use config::Ssr;
pub(crate) use error::SsrFailure;
pub use error::SsrStartError;
pub use policy::{SsrContext, SsrOverride, SsrRouteExt};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WireSsrResponse {
    pub(crate) head: Vec<String>,
    pub(crate) body: String,
}

#[derive(Clone, Debug)]
pub(crate) struct SsrResponse {
    pub(crate) head: Vec<String>,
    pub(crate) body: String,
}

impl From<WireSsrResponse> for SsrResponse {
    fn from(value: WireSsrResponse) -> Self {
        Self {
            head: value.head,
            body: value.body,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SsrEndpoints {
    pub(crate) render: axum::http::Uri,
    pub(crate) health: Option<axum::http::Uri>,
    pub(crate) shutdown: Option<axum::http::Uri>,
}

impl SsrEndpoints {
    pub(crate) fn node(base: &str) -> Result<Self, SsrStartError> {
        Ok(Self {
            render: endpoint(base, "/render")?,
            health: Some(endpoint(base, "/health")?),
            shutdown: Some(endpoint(base, "/shutdown")?),
        })
    }

    pub(crate) fn vite(base: &str) -> Result<Self, SsrStartError> {
        Ok(Self {
            render: endpoint(base, "/__inertia_ssr")?,
            health: None,
            shutdown: None,
        })
    }
}

fn endpoint(base: &str, path: &str) -> Result<axum::http::Uri, SsrStartError> {
    let base = base.trim_end_matches('/');
    let value = format!("{base}{path}");
    let uri =
        value
            .parse::<axum::http::Uri>()
            .map_err(|source| SsrStartError::InvalidEndpoint {
                endpoint: value.clone(),
                source,
            })?;
    if uri.scheme_str() != Some("http") || uri.authority().is_none() {
        return Err(SsrStartError::UnsupportedEndpoint(value));
    }
    Ok(uri)
}
