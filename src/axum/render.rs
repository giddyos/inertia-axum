//! Axum response rendering and compatibility re-exports.

pub use super::error::InertiaError;
pub use super::extract::InertiaRequest;
pub use super::shared::{SharedProps, SharedRequest};
pub use super::version::{InertiaVersion, VersionLayer, VersionService};
pub use crate::HtmlResponseContext;

use super::response_headers::{
    add_vary_header, conflict_response, is_write_method, redirect_response,
};
use crate::html::html_response_context;
use crate::{Inertia, IntoPageProps, Location, Redirect, RequestContext, X_INERTIA_HEADER};
use axum::http::{HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;

impl InertiaRequest {
    /// Returns `true` when the request includes the `X-Inertia` header.
    pub fn is_inertia(&self) -> bool {
        self.context.is_inertia()
    }

    /// Returns the parsed request context.
    pub fn context(&self) -> &RequestContext {
        &self.context
    }

    /// Returns the request URI used as the default page-object URL.
    pub fn uri(&self) -> &str {
        &self.uri
    }

    /// Returns the current asset version installed by [`VersionLayer`].
    pub fn asset_version(&self) -> Option<&str> {
        self.version.as_ref().map(InertiaVersion::as_str)
    }

    /// Converts an [`Inertia`] value into an Axum response.
    ///
    /// Inertia requests receive a JSON page object with `X-Inertia: true`.
    /// Direct browser requests are rendered through `html_response`.
    pub fn render<T, F, R>(
        &self,
        inertia: Inertia<T>,
        html_response: F,
    ) -> Result<Response, InertiaError>
    where
        T: IntoPageProps,
        F: FnOnce(HtmlResponseContext) -> R,
        R: IntoResponse,
    {
        let partial_reload_enabled = self.method == Method::GET;
        let mut draft = inertia.into_page_draft(
            &self.uri,
            self.version.as_ref().map(InertiaVersion::clone_arc),
            &self.context,
            partial_reload_enabled,
        )?;

        if let Some(shared_props) = &self.shared_props {
            if !shared_props.is_empty() {
                let shared_request = SharedRequest::new(
                    &self.context,
                    &self.method,
                    &self.uri,
                    self.asset_version(),
                );
                shared_props.merge_into(&shared_request, &mut draft)?;
            }
        }

        let page = draft.finish();

        if self.context.is_inertia() {
            let mut response = Json(page).into_response();
            response
                .headers_mut()
                .insert(X_INERTIA_HEADER, HeaderValue::from_static("true"));
            add_vary_header(&mut response);
            Ok(response)
        } else {
            let context = html_response_context(&page)?;
            let mut response = html_response(context).into_response();
            add_vary_header(&mut response);
            Ok(response)
        }
    }

    /// Converts an external Inertia location visit into an Axum response.
    ///
    /// Inertia requests receive `409 Conflict` with `X-Inertia-Location`,
    /// or `X-Inertia-Redirect` for fragment destinations.
    /// Direct browser requests fall back to a method-aware redirect.
    pub fn location(&self, location: Location) -> Result<Response, InertiaError> {
        if self.context.is_inertia() {
            conflict_response(location.url())
        } else if is_write_method(&self.method) {
            redirect_response(StatusCode::SEE_OTHER, location.url())
        } else {
            redirect_response(StatusCode::FOUND, location.url())
        }
    }

    /// Converts a method-aware redirect into an Axum response.
    pub fn redirect(&self, redirect: Redirect) -> Result<Response, InertiaError> {
        if is_write_method(&self.method) {
            redirect_response(StatusCode::SEE_OTHER, redirect.url())
        } else {
            redirect_response(StatusCode::FOUND, redirect.url())
        }
    }
}
