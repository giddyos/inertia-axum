//! Request-aware finalization without extractor or Tower concerns.

use crate::{
    app::InertiaApp,
    axum::response_headers::{
        add_vary_header, conflict_response, is_write_method, redirect_response,
    },
    html::html_response_context,
    response::{PendingPage, PendingResponse},
    root::{MountMarkup, RootContext},
    visit::Visit,
    X_INERTIA_HEADER,
};
use axum::{
    http::{HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Clone)]
pub(crate) struct Engine {
    app: InertiaApp,
}

impl Engine {
    pub(crate) fn new(app: InertiaApp) -> Self {
        Self { app }
    }

    pub(crate) fn finalize(&self, visit: &Visit, pending: PendingResponse) -> Response {
        let result = match pending {
            PendingResponse::Page(page) => self.finalize_page(visit, *page),
            PendingResponse::Redirect(redirect) => {
                let status = if is_write_method(&visit.method) {
                    StatusCode::SEE_OTHER
                } else {
                    StatusCode::FOUND
                };
                redirect_response(status, redirect.resolve(visit.referer.as_deref()))
            }
            PendingResponse::Location(location) => {
                if visit.is_inertia() {
                    conflict_response(location.url())
                } else {
                    let status = if is_write_method(&visit.method) {
                        StatusCode::SEE_OTHER
                    } else {
                        StatusCode::FOUND
                    };
                    redirect_response(status, location.url())
                }
            }
        };
        result.unwrap_or_else(crate::axum::error::internal_error_response)
    }

    fn finalize_page(
        &self,
        visit: &Visit,
        pending: PendingPage,
    ) -> Result<Response, crate::axum::InertiaError> {
        let draft = pending.inertia.into_page_draft_version(
            &visit.uri,
            self.app.inner.assets.version.clone(),
            &visit.context,
            visit.method == axum::http::Method::GET,
        )?;
        let page = draft.finish();
        finalize_page_object(page, visit.is_inertia(), pending.status, |serialized| {
            let assets = self.app.inner.assets.tags.clone();
            let mount = MountMarkup::new(serialized.data_page());
            let html = self
                .app
                .inner
                .root
                .render(RootContext::new(&assets, &mount))
                .map_err(crate::axum::InertiaError::root)?;
            Ok(Html(html).into_response())
        })
    }
}

pub(crate) fn finalize_page_object<T, F>(
    page: T,
    is_inertia: bool,
    status: StatusCode,
    html: F,
) -> Result<Response, crate::axum::InertiaError>
where
    T: Serialize,
    F: FnOnce(crate::HtmlResponseContext) -> Result<Response, crate::axum::InertiaError>,
{
    let mut response = if is_inertia {
        let mut response = Json(page).into_response();
        response
            .headers_mut()
            .insert(X_INERTIA_HEADER, HeaderValue::from_static("true"));
        response
    } else {
        html(html_response_context(&page)?)?
    };
    *response.status_mut() = status;
    add_vary_header(&mut response);
    Ok(response)
}
