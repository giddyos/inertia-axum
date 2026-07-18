//! Axum response rendering and compatibility re-exports.

pub use super::error::InertiaError;
pub use super::extract::InertiaRequest;
pub use super::shared::{SharedProps, SharedRequest};
pub use super::version::{InertiaVersion, VersionLayer, VersionService};
pub use crate::HtmlResponseContext;

use super::response_headers::{conflict_response, is_write_method, redirect_response};
use crate::{IntoPageProps, RequestContext};
use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use inertia_core::{Inertia, Location, Redirect};

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

        finalize_page_object(page, self.context.is_inertia(), StatusCode::OK, |context| {
            Ok(html_response(context).into_response())
        })
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
            redirect_response(
                StatusCode::SEE_OTHER,
                redirect.resolve(self.referer.as_deref()),
            )
        } else {
            redirect_response(StatusCode::FOUND, redirect.resolve(self.referer.as_deref()))
        }
    }
}

fn finalize_page_object<T, F>(
    page: T,
    is_inertia: bool,
    status: StatusCode,
    html: F,
) -> Result<Response, InertiaError>
where
    T: serde::Serialize,
    F: FnOnce(crate::HtmlResponseContext) -> Result<Response, InertiaError>,
{
    let mut response = if is_inertia {
        let mut response = axum::Json(page).into_response();
        response.headers_mut().insert(
            crate::X_INERTIA_HEADER,
            axum::http::HeaderValue::from_static("true"),
        );
        response
    } else {
        html(inertia_core::__private::html_response_context(&page)?)?
    };
    *response.status_mut() = status;
    super::response_headers::add_vary_header(&mut response);
    Ok(response)
}
